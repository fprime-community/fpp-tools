use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::global_state::GlobalState;
use fpp_analysis::Analysis;
use fpp_analysis::semantics::Symbol;
use fpp_analysis::semantics::state_machine::StateMachineSymbol;
use fpp_ast::{
    DefStateMachine, Expr, ExprKind, Ident, MoveWalkable, Node, PortInstanceIdentifier,
    SpecPortMatching, TlmChannelIdentifier, Visitor, Walkable,
};
use fpp_core::{LineCol, LineIndex, SourceFile, SourceFileData};
use fpp_lsp_parser::{SyntaxKind, SyntaxNode, SyntaxToken, TextRange, VisitorResult};
use lsp_types::{SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u32)]
pub enum SemanticTokenKind {
    Module,
    Topology,
    System,
    Component,
    Interface,
    ComponentInstance,
    Constant,
    EnumConstant,
    StructMember,
    GraphGroup,
    PortInstance,
    Port,
    AbstractType,
    AliasType,
    ArrayType,
    EnumType,
    StructType,
    PrimitiveType,
    FormalParameter,
    Command,
    Event,
    Telemetry,
    Parameter,
    DataProduct,

    StateMachine,
    StateMachineInstance,
    TelemetryPacketSet,
    TelemetryPacket,

    Action,
    Guard,
    Signal,
    State,
    Choice,

    Annotation,
    Comment,
    Number,
    #[allow(dead_code)]
    String,
    #[allow(dead_code)]
    Keyword,
}

impl SemanticTokenKind {
    pub const TOKEN_TYPES: [SemanticTokenType; 38] = [
        SemanticTokenType::new("module"),
        SemanticTokenType::new("topology"),
        SemanticTokenType::new("system"),
        SemanticTokenType::new("component"),
        SemanticTokenType::new("interface"),
        SemanticTokenType::new("componentInstance"),
        SemanticTokenType::new("constant"),
        SemanticTokenType::new("enumConstant"),
        SemanticTokenType::new("structMember"),
        SemanticTokenType::new("graphGroup"),
        SemanticTokenType::new("portInstance"),
        SemanticTokenType::new("port"),
        SemanticTokenType::new("abstractType"),
        SemanticTokenType::new("aliasType"),
        SemanticTokenType::new("arrayType"),
        SemanticTokenType::new("enumType"),
        SemanticTokenType::new("structType"),
        SemanticTokenType::new("primitiveType"),
        SemanticTokenType::new("formalParameter"),
        SemanticTokenType::new("command"),
        SemanticTokenType::new("event"),
        SemanticTokenType::new("telemetry"),
        SemanticTokenType::new("parameter"),
        SemanticTokenType::new("dataProduct"),
        SemanticTokenType::new("stateMachine"),
        SemanticTokenType::new("stateMachineInstance"),
        SemanticTokenType::new("telemetryPacketSet"),
        SemanticTokenType::new("telemetryPacket"),
        SemanticTokenType::new("action"),
        SemanticTokenType::new("guard"),
        SemanticTokenType::new("signal"),
        SemanticTokenType::new("state"),
        SemanticTokenType::new("choice"),
        SemanticTokenType::new("annotation"),
        SemanticTokenType::new("comment"),
        SemanticTokenType::new("number"),
        SemanticTokenType::new("string"),
        SemanticTokenType::new("keyword"),
    ];

    pub const TOKEN_MODIFIERS: [SemanticTokenModifier; 0] = [];

    pub fn type_and_modifier(self) -> (u32, u32) {
        (self as u32, 0)
    }
}

pub(crate) struct SemanticTokensState {
    analysis: Arc<Analysis>,
    lines: LineIndex,
    raw: Vec<(TextRange, SemanticTokenKind)>,
}

static TOKEN_RESULT_COUNTER: AtomicU32 = AtomicU32::new(1);

impl SemanticTokensState {
    fn new(analysis: Arc<Analysis>, text: &str) -> SemanticTokensState {
        SemanticTokensState {
            analysis,
            lines: LineIndex::new(text),
            raw: Default::default(),
        }
    }

    pub(crate) fn finish(mut self, filter_range: Option<lsp_types::Range>) -> SemanticTokens {
        let id = TOKEN_RESULT_COUNTER
            .fetch_add(1, Ordering::SeqCst)
            .to_string();

        let mut tokens = SemanticTokens {
            result_id: Some(id),
            data: vec![],
        };

        // The semantic token stream forbids overlapping ranges.
        self.raw
            .sort_by(|(a, _), (b, _)| a.start().cmp(&b.start()).then(b.end().cmp(&a.end())));
        let mut deduped: Vec<(TextRange, SemanticTokenKind)> = Vec::with_capacity(self.raw.len());
        let mut last_end = TextRange::default().end();
        for (range, kind) in self.raw {
            if range.start() < last_end {
                continue;
            }
            last_end = range.end();
            deduped.push((range, kind));
        }
        self.raw = deduped;

        let mut last = LineCol { line: 0, col: 0 };

        let filter_range = filter_range.and_then(|filter_range| {
            Some(TextRange::new(
                self.lines.offset(LineCol {
                    line: filter_range.start.line,
                    col: filter_range.start.character,
                })?,
                self.lines.offset(LineCol {
                    line: filter_range.end.line,
                    col: filter_range.end.character,
                })?,
            ))
        });

        for (range, kind) in self.raw {
            if let Some(filter_range) = filter_range
                && filter_range.intersect(range).is_none()
            {
                continue;
            }

            let (Some(start), Some(end)) = (
                self.lines.try_line_col(range.start()),
                self.lines.try_line_col(range.end()),
            ) else {
                continue;
            };

            let (token_type, token_modifiers_bitset) = kind.type_and_modifier();

            // LSP semantic tokens cannot span multiple lines (the client
            // advertises multilineTokenSupport: false). A range that crosses a
            // line boundary is emitted as one token per line it covers.
            for line in start.line..=end.line {
                let start_col = if line == start.line { start.col } else { 0 };
                let end_col = if line == end.line {
                    end.col
                } else {
                    // The column at the end of this line, excluding the newline.
                    match self.lines.line(line) {
                        Some(line_range) => self.lines.line_col(line_range.end()).col,
                        None => start_col,
                    }
                };

                if start_col >= end_col {
                    // Empty segment (e.g. a range that ends exactly at a line
                    // break has a zero-width piece on the following line).
                    continue;
                }

                let delta_line = line - last.line;
                let delta_start = if delta_line == 0 {
                    // Same line, offset the start position. The dedup above
                    // guarantees non-overlapping, start-ordered ranges, so this
                    // invariant should always hold
                    if !(start_col > last.col || tokens.data.is_empty()) {
                        debug_assert!(
                            false,
                            "semantic tokens overlap: {}:{} (last) | {}:{}..{}:{} ({:?})",
                            last.line, last.col, line, start_col, line, end_col, kind
                        );
                        continue;
                    }

                    start_col - last.col
                } else {
                    // Token is on a different line, don't alter the column
                    start_col
                };

                last = LineCol {
                    line,
                    col: start_col,
                };
                tokens.data.push(SemanticToken {
                    delta_line,
                    delta_start,
                    length: end_col - start_col,
                    token_type,
                    token_modifiers_bitset,
                });
            }
        }

        tokens
    }

    #[inline]
    fn add_text_range(&mut self, range: TextRange, kind: SemanticTokenKind) {
        self.raw.push((range, kind));
    }

    fn add_token(&mut self, token: &SyntaxToken, kind: SemanticTokenKind) {
        self.add_text_range(token.text_range(), kind);
    }

    fn add_node(&mut self, node: &SyntaxNode, kind: SemanticTokenKind) {
        self.add_text_range(node.text_range(), kind);
    }
}

struct SemanticTokenVisitor {}
impl fpp_lsp_parser::Visitor for SemanticTokenVisitor {
    type State = SemanticTokensState;

    fn visit_node(&self, state: &mut Self::State, node: &SyntaxNode) -> VisitorResult {
        match node.kind() {
            // These are typed by definitions above them
            SyntaxKind::NAME => {
                if let Some(parent_node_kind) = node.parent().map(|f| f.kind()) {
                    let name_kind = match parent_node_kind {
                        SyntaxKind::DEF_ABSTRACT_TYPE => SemanticTokenKind::AbstractType,
                        SyntaxKind::DEF_ALIAS_TYPE => SemanticTokenKind::AliasType,
                        SyntaxKind::DEF_ARRAY => SemanticTokenKind::ArrayType,
                        SyntaxKind::DEF_ENUM => SemanticTokenKind::EnumType,
                        SyntaxKind::DEF_STRUCT => SemanticTokenKind::StructType,
                        SyntaxKind::FORMAL_PARAM => SemanticTokenKind::FormalParameter,
                        SyntaxKind::DEF_COMPONENT => SemanticTokenKind::Component,
                        SyntaxKind::DEF_COMPONENT_INSTANCE => SemanticTokenKind::ComponentInstance,
                        SyntaxKind::DEF_ENUM_CONSTANT => SemanticTokenKind::EnumConstant,
                        SyntaxKind::DEF_CONSTANT => SemanticTokenKind::Constant,
                        SyntaxKind::DEF_INTERFACE => SemanticTokenKind::Interface,
                        SyntaxKind::DEF_TOPOLOGY => SemanticTokenKind::Topology,
                        SyntaxKind::DEF_SYSTEM => SemanticTokenKind::System,
                        SyntaxKind::STRUCT_MEMBER => SemanticTokenKind::StructMember,
                        SyntaxKind::SPEC_CONNECTION_GRAPH_DIRECT => SemanticTokenKind::GraphGroup,
                        SyntaxKind::SPEC_TOP_PORT => SemanticTokenKind::PortInstance,
                        SyntaxKind::SPEC_PORT_INSTANCE_GENERAL => SemanticTokenKind::PortInstance,
                        SyntaxKind::SPEC_PORT_INSTANCE_SPECIAL => SemanticTokenKind::PortInstance,
                        SyntaxKind::SPEC_PORT_INSTANCE_INTERNAL => SemanticTokenKind::PortInstance,
                        SyntaxKind::SPEC_COMMAND => SemanticTokenKind::Command,
                        SyntaxKind::SPEC_EVENT => SemanticTokenKind::Event,
                        SyntaxKind::SPEC_PARAM => SemanticTokenKind::Parameter,
                        SyntaxKind::SPEC_RECORD => SemanticTokenKind::DataProduct,
                        SyntaxKind::SPEC_CONTAINER => SemanticTokenKind::DataProduct,
                        SyntaxKind::SPEC_TELEMETRY => SemanticTokenKind::Telemetry,
                        SyntaxKind::DEF_MODULE => SemanticTokenKind::Module,
                        SyntaxKind::DEF_PORT => SemanticTokenKind::Port,
                        SyntaxKind::DEF_ACTION => SemanticTokenKind::Action,
                        SyntaxKind::DEF_GUARD => SemanticTokenKind::Guard,
                        SyntaxKind::DEF_SIGNAL => SemanticTokenKind::Signal,
                        SyntaxKind::DEF_STATE => SemanticTokenKind::State,
                        SyntaxKind::DEF_CHOICE => SemanticTokenKind::Choice,
                        SyntaxKind::DEF_STATE_MACHINE => SemanticTokenKind::StateMachine,
                        SyntaxKind::TLM_PACKET_SET => SemanticTokenKind::TelemetryPacketSet,
                        SyntaxKind::SPEC_TLM_PACKET => SemanticTokenKind::TelemetryPacket,
                        SyntaxKind::SPEC_STATE_MACHINE_INSTANCE => {
                            SemanticTokenKind::StateMachineInstance
                        }
                        // We do not recognize this name's parent rule
                        _ => return VisitorResult::Next,
                    };

                    state.add_node(node, name_kind);
                }

                VisitorResult::Next
            }

            // Keep going deeper to look at children
            _ => VisitorResult::Recurse,
        }
    }

    fn visit_token(&self, state: &mut Self::State, token: &SyntaxToken) {
        let kind = match token.kind() {
            pk if pk.is_type_primitive_keyword() => SemanticTokenKind::PrimitiveType,
            SyntaxKind::POST_ANNOTATION | SyntaxKind::PRE_ANNOTATION => {
                SemanticTokenKind::Annotation
            }
            // TODO(tumbar) Port all the tmLanguage tokens/highlights to LSP
            // keyword if keyword.is_keyword() => SemanticTokenKind::Keyword,
            SyntaxKind::COMMENT => SemanticTokenKind::Comment,
            SyntaxKind::LITERAL_FLOAT | SyntaxKind::LITERAL_INT => SemanticTokenKind::Number,
            // SyntaxKind::LITERAL_STRING => SemanticTokenKind::String,
            _ => return,
        };

        state.add_token(token, kind);
    }
}

struct SemanticUses<'ast> {
    source_file: &'ast SourceFileData,
    context: &'ast GlobalState,
}

impl<'ast> SemanticUses<'ast> {
    /// The text range of a node, if it lies in the file being tokenized.
    #[inline]
    fn node_range(&self, node: &fpp_core::Node) -> Option<TextRange> {
        let span = self.context.context.node_get_span(node);
        let span_data = self.context.context.span_get(&span);
        let src_file_handle = span_data.file.upgrade().unwrap().handle;

        if src_file_handle == self.source_file.handle {
            Some(TextRange::new(
                span_data.start.into(),
                (span_data.start + span_data.length).into(),
            ))
        } else {
            None
        }
    }

    #[inline]
    fn mark_node(
        &self,
        a: &mut SemanticTokensState,
        node: fpp_core::Node,
        semantic_kind: SemanticTokenKind,
    ) {
        if let Some(range) = self.node_range(&node) {
            a.add_text_range(range, semantic_kind);
        }
    }

    #[inline]
    fn mark_use(
        &self,
        a: &mut SemanticTokensState,
        node: fpp_core::Node,
        use_node: fpp_core::Node,
    ) {
        if let Some(symbol) = a.analysis.use_def_map.get(&use_node) {
            let semantic_kind = match symbol {
                Symbol::AbsType(_) => SemanticTokenKind::AbstractType,
                Symbol::AliasType(_) => SemanticTokenKind::AliasType,
                Symbol::Array(_) => SemanticTokenKind::ArrayType,
                Symbol::Component(_) => SemanticTokenKind::Component,
                Symbol::ComponentInstance(_) => SemanticTokenKind::ComponentInstance,
                Symbol::Constant(_) => SemanticTokenKind::Constant,
                Symbol::Enum(_) => SemanticTokenKind::EnumType,
                Symbol::EnumConstant(_) => SemanticTokenKind::EnumConstant,
                Symbol::Interface(_) => SemanticTokenKind::Interface,
                Symbol::Module(_) => SemanticTokenKind::Module,
                Symbol::Port(_) => SemanticTokenKind::Port,
                Symbol::StateMachine(_) => SemanticTokenKind::StateMachine,
                Symbol::Struct(_) => SemanticTokenKind::StructType,
                Symbol::System(_) => SemanticTokenKind::System,
                Symbol::Topology(_) => SemanticTokenKind::Topology,
            };

            self.mark_node(a, node, semantic_kind);
        }
    }
}

impl<'ast> Visitor<'ast> for SemanticUses<'ast> {
    type Break = ();
    type State = SemanticTokensState;

    // Walk all nodes deeply and collect up scope where relevant
    fn super_visit(&self, a: &mut Self::State, node: Node<'ast>) -> ControlFlow<Self::Break> {
        node.walk(a, self)
    }

    fn visit_expr(&self, a: &mut Self::State, node: &'ast Expr) -> ControlFlow<Self::Break> {
        match &node.kind {
            ExprKind::Ident(_) => {
                self.mark_use(a, node.node_id, node.node_id);
                ControlFlow::Continue(())
            }
            ExprKind::Dot { e, id } => {
                self.mark_use(a, id.node_id, node.node_id);
                e.walk(a, self)
            }
            _ => node.walk(a, self),
        }
    }

    fn visit_ident(&self, a: &mut Self::State, node: &'ast Ident) -> ControlFlow<Self::Break> {
        self.mark_use(a, node.node_id, node.node_id);
        ControlFlow::Continue(())
    }

    // TODO(tumbar) We should add these to the analysis and mark these are true uses

    fn visit_tlm_channel_identifier(
        &self,
        a: &mut Self::State,
        node: &'ast TlmChannelIdentifier,
    ) -> ControlFlow<Self::Break> {
        // Channel name is not a use, we need to mark it manually
        self.mark_node(a, node.channel_name.node_id, SemanticTokenKind::Telemetry);
        node.component_instance.walk(a, self)
    }

    fn visit_port_instance_identifier(
        &self,
        a: &mut Self::State,
        node: &'ast PortInstanceIdentifier,
    ) -> ControlFlow<Self::Break> {
        // Port instance is not a use, we need to mark it manually
        self.mark_node(a, node.port_name.node_id, SemanticTokenKind::PortInstance);
        node.interface_instance.walk(a, self)
    }

    fn visit_spec_port_matching(
        &self,
        a: &mut Self::State,
        node: &'ast SpecPortMatching,
    ) -> ControlFlow<Self::Break> {
        // The two port names in a match specifier (`match a with b`) are bare
        // identifiers, not uses in the use-def map, so mark them manually as
        // port instances (matching how connection port names are colored).
        self.mark_node(a, node.port1.node_id, SemanticTokenKind::PortInstance);
        self.mark_node(a, node.port2.node_id, SemanticTokenKind::PortInstance);
        ControlFlow::Continue(())
    }

    fn visit_def_state_machine(
        &self,
        a: &mut Self::State,
        node: &'ast DefStateMachine,
    ) -> ControlFlow<Self::Break> {
        // Uses inside a state machine (actions, guards, signals, states,
        // choices) are resolved into the state machine's own use-def map during
        // state machine analysis, not the global one. Color them the same way
        // the outer use-def map is colored: look up each use node's resolved
        // symbol and mark it by kind.
        //
        // Resolve each use node to a (range, kind). Qualified state uses (`A.B`)
        // record a use on both the whole qualified-identifier node and its
        // qualifier, whose ranges nest; the semantic token stream forbids
        // overlaps, so we keep only the widest range in any overlapping group
        // (all segments of a state path share the `State` color anyway).
        let mut ranges: Vec<(TextRange, SemanticTokenKind)> = a
            .analysis
            .symbol_map
            .get(&node.node_id)
            .and_then(|symbol| a.analysis.state_machine_map.get(symbol))
            .map(|state_machine| {
                state_machine
                    .sma
                    .use_def_map
                    .iter()
                    .filter_map(|(use_node, sm_symbol)| {
                        Some((self.node_range(use_node)?, sm_symbol_kind(sm_symbol)))
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Sort by start, then widest first, and drop any range that overlaps
        // one already kept.
        ranges.sort_by(|(a, _), (b, _)| a.start().cmp(&b.start()).then(b.end().cmp(&a.end())));
        let mut last_end = TextRange::default().end();
        for (range, kind) in ranges {
            if range.start() < last_end {
                continue;
            }
            last_end = range.end();
            a.add_text_range(range, kind);
        }

        // Walk the children so ordinary uses inside the state machine still get
        // colored through the global use-def map — in particular the type names
        // on action/guard/signal definitions (`action a: SomeType`). The state
        // machine's own action/guard/signal/state uses are not in the global
        // map, so this walk does not re-mark (or overlap) the ranges above.
        node.walk(a, self)
    }
}

/// The semantic token color for a state machine symbol.
fn sm_symbol_kind(symbol: &StateMachineSymbol) -> SemanticTokenKind {
    match symbol {
        StateMachineSymbol::Action(_) => SemanticTokenKind::Action,
        StateMachineSymbol::Guard(_) => SemanticTokenKind::Guard,
        StateMachineSymbol::Choice(_) => SemanticTokenKind::Choice,
        StateMachineSymbol::Signal(_) => SemanticTokenKind::Signal,
        StateMachineSymbol::State(_) => SemanticTokenKind::State,
    }
}

pub(crate) fn compute(
    global_state: &GlobalState,
    source_file: Option<SourceFile>,
    text: &str,
    parse: &fpp_lsp_parser::Parse,
) -> SemanticTokensState {
    let mut state = SemanticTokensState::new(global_state.analysis.clone(), text);
    parse.visit(&mut state, &SemanticTokenVisitor {});

    if let Some(source_file) = source_file {
        let parent_file = global_state.parent_file(source_file);
        let cache = global_state.cache.get(&parent_file).unwrap();

        let _ = SemanticUses {
            source_file: global_state.context.file_get(&source_file),
            context: global_state,
        }
        .visit_trans_unit(&mut state, &cache.ast);
    }

    state
}
