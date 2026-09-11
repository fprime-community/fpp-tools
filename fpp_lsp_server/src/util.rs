use crate::diagnostics::LspDiagnosticsEmitter;
use crate::global_state::GlobalState;
use fpp_analysis::semantics::state_machine::{StateMachine, StateMachineSymbol};
use fpp_analysis::semantics::{
    Component, Direction, NameGroup, PortInstance, PortInstanceType, PortInterface, Scope, Symbol,
    SymbolInterface, Type,
};
use fpp_ast::{AstNode, FormalParam, FormalParamKind, MoveWalkable, Name, Node, Visitor};
use fpp_core::{BytePos, CompilerContext, LineCol, SourceFile};
use fpp_lsp_parser::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, TextSize};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemLabelDetails, Documentation, Hover,
    HoverContents, Location, MarkupContent, MarkupKind, Position, Range, Uri,
};
use serde::de::DeserializeOwned;
use std::ops::{ControlFlow, Deref};
use std::str::FromStr;
use std::sync::Arc;

pub fn from_json<T: DeserializeOwned>(
    what: &'static str,
    json: &serde_json::Value,
) -> anyhow::Result<T> {
    serde_json::from_value(json.clone())
        .map_err(|e| anyhow::format_err!("Failed to deserialize {what}: {e}; {json}"))
}

pub(crate) struct FindPositionVisitor<'a> {
    source_file: SourceFile,
    looking_for: BytePos,
    context: &'a CompilerContext<LspDiagnosticsEmitter>,
}

impl<'ast> Visitor<'ast> for FindPositionVisitor<'ast> {
    type Break = ();
    type State = Vec<Node<'ast>>;

    /// The default node visiting before.
    /// By default, this will just continue without visiting the children of `node`
    fn super_visit(&self, a: &mut Self::State, node: Node<'ast>) -> ControlFlow<Self::Break> {
        let span = self
            .context
            .span_get(&self.context.node_get_span(&node.id()));

        let src_file: SourceFile = span.file.upgrade().unwrap().as_ref().into();

        if src_file == self.source_file {
            // Check if this node spans the range we are looking for
            if span.start <= self.looking_for && span.start + span.length >= self.looking_for {
                // Depth first
                let out = node.walk(a, self);

                a.push(node);
                out
            } else {
                // This node does not span the range
                // We don't need to walk it since it's children won't span it either
                ControlFlow::Continue(())
            }
        } else {
            // The files don't match
            // We could be looking for something inside an include
            // Keep recursing
            match node {
                Node::DefAction(_) => node.walk(a, self),
                Node::DefComponent(_) => node.walk(a, self),
                Node::DefModule(_) => node.walk(a, self),
                Node::DefState(_) => node.walk(a, self),
                Node::DefStateMachine(_) => node.walk(a, self),
                Node::DefTopology(_) => node.walk(a, self),
                Node::SpecInclude(_) => node.walk(a, self),
                _ => ControlFlow::Continue(()),
            }
        }
    }
}

pub fn nodes_at_offset<'a>(
    state: &'a GlobalState,
    document: &Uri,
    offset: BytePos,
) -> Option<Vec<Node<'a>>> {
    let files = state.files.get(document.as_str())?;

    Some(
        files
            .iter()
            .flat_map(|file| {
                let cache = state.cache.get(&state.parent_file(*file)).unwrap();

                let visitor = FindPositionVisitor {
                    source_file: *file,
                    looking_for: offset,
                    context: &state.context,
                };

                let mut out = vec![];
                let _ = visitor.visit_trans_unit(&mut out, &cache.ast);
                out
            })
            .collect(),
    )
}

#[inline]
pub fn position_to_offset(state: &GlobalState, document: &Uri, position: &Position) -> BytePos {
    state
        .vfs
        .get_lines(document.as_str())
        .unwrap()
        .offset(LineCol {
            line: position.line,
            col: position.character,
        })
        .unwrap()
        .into()
}

pub(crate) fn symbol_at_position<'a>(
    state: &'a GlobalState,
    document: &Uri,
    position: BytePos,
) -> Option<(Node<'a>, Symbol)> {
    let nodes = nodes_at_offset(state, document, position)?;

    nodes.iter().find_map(|node| {
        state
            .analysis
            .use_def_map
            .get(&node.id())
            .map(|def| (*node, def.clone()))
    })
}

fn node_to_range(state: &GlobalState, node: fpp_core::Node) -> Range {
    let span = state.context.span_get(&state.context.node_get_span(&node));
    let file = span.file.upgrade().unwrap();

    let start = file.position(span.start);
    let end = file.position(span.start + span.length);

    Range {
        start: Position {
            line: start.line(),
            character: start.column(),
        },
        end: Position {
            line: end.line(),
            character: end.column(),
        },
    }
}

pub fn node_to_location(state: &GlobalState, node: fpp_core::Node) -> Location {
    let span = state.context.span_get(&state.context.node_get_span(&node));
    let file = span.file.upgrade().unwrap();

    let start = file.position(span.start);
    let end = file.position(span.start + span.length);

    Location {
        uri: Uri::from_str(&file.uri).unwrap(),
        range: Range {
            start: Position {
                line: start.line(),
                character: start.column(),
            },
            end: Position {
                line: end.line(),
                character: end.column(),
            },
        },
    }
}

fn symbol_kind_name(symbol: &Symbol) -> &'static str {
    match symbol {
        Symbol::AbsType(_) => "Abstract Type",
        Symbol::AliasType(_) => "Type Alias",
        Symbol::ArrayType(_) => "Array",
        Symbol::Component(_) => "Component",
        Symbol::ComponentInstance(_) => "Component Instance",
        Symbol::Constant(_) => "Constant",
        Symbol::EnumType(_) => "Enum",
        Symbol::EnumConstant(_) => "Enum Constant",
        Symbol::Interface(_) => "Interface",
        Symbol::Module(_) => "Module",
        Symbol::Port(_) => "Port",
        Symbol::StateMachine(_) => "State Machine",
        Symbol::StructType(_) => "Struct",
        Symbol::System(_) => "System",
        Symbol::Topology(_) => "Topology",
    }
}

pub fn hover_for_symbol(state: &GlobalState, hover_node: Node, symbol: &Symbol) -> Hover {
    let node_data = state.context.node_get(&symbol.node());
    let symbol_kind = symbol_kind_name(symbol);

    // Convert the name into a fully qualified name by following the parent symbols
    let mut qualified_name = vec![symbol];
    let mut current = symbol;
    loop {
        match state.analysis.parent_symbol_map.get(current) {
            None => break,
            Some(parent) => {
                qualified_name.push(parent);
                current = parent;
            }
        }
    }

    qualified_name.reverse();
    let qualified_idents: Vec<&str> = qualified_name
        .into_iter()
        .map(|n| n.name().data.as_str())
        .collect();
    let qual_ident = qualified_idents.join(".");

    let symbol_kind_line = state.analysis.value_map.get(&symbol.node()).map_or_else(
        || format!("({symbol_kind}) {qual_ident}"),
        |v| format!("({symbol_kind}) {qual_ident} = {v}"),
    );

    let markdown_lines: Vec<String> = node_data
        .pre_annotation
        .clone()
        .into_iter()
        .chain(vec!["".to_string(), symbol_kind_line, "".to_string()])
        .chain(node_data.post_annotation.clone())
        .collect();

    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: markdown_lines.join("\n").trim().to_string(),
        }),
        range: Some(node_to_range(state, hover_node.id())),
    }
}

pub fn hover_for_node(state: &GlobalState, hover_node: &Name, def_node: Node) -> Option<Hover> {
    let symbol_kind = match def_node {
        Node::DefAbsType(_) => "Abstract Type",
        Node::DefAliasType(_) => "Type Alias",
        Node::DefArray(_) => "Array",
        Node::DefComponent(_) => "Component",
        Node::DefComponentInstance(_) => "Component Instance",
        Node::DefConstant(_) => "Constant",
        Node::DefEnum(_) => "Enum",
        Node::DefEnumConstant(_) => "Enum Constant",
        Node::DefInterface(_) => "Interface",
        Node::DefModule(_) => "Module",
        Node::DefPort(_) => "Port",
        Node::DefStateMachine(_) => "State Machine",
        Node::DefStruct(_) => "Struct",
        Node::DefTopology(_) => "Topology",
        Node::DefChoice(_) => "Choice",
        Node::DefGuard(_) => "Guard",
        Node::DefSignal(_) => "Signal",
        Node::DefState(_) => "State",
        Node::SpecCommand(_) => "Command",
        Node::SpecDirectConnectionGraph(_) => "Direct Connection Graph",
        Node::SpecPatternConnectionGraph(_) => "Pattern Connection Graph",
        Node::SpecContainer(_) => "Container",
        Node::SpecEvent(_) => "Event",
        Node::SpecGeneralPortInstance(_) => "Port Instance",
        Node::SpecParam(_) => "Parameter",
        Node::SpecRecord(_) => "Record",
        Node::SpecSpecialPortInstance(_) => "Special Port Instance",
        Node::SpecStateMachineInstance(_) => "State Machine Instance",
        Node::SpecTlmChannel(_) => "Telemetry Channel",
        Node::SpecTlmPacket(_) => "Telemetry Packet",
        Node::SpecTlmPacketSet(_) => "Telemetry Packet Set",
        Node::SpecTopPort(_) => "Topology Port",
        Node::FormalParam(_) => "Formal Parameter",
        _ => return None,
    };

    let node_data = state.context.node_get(&def_node.id());

    // Convert the name into a fully qualified name by following the parent symbols
    let qual_ident = {
        if let Some(symbol) = state.analysis.symbol_map.get(&def_node.id()) {
            let mut qualified_name = vec![symbol];
            let mut current = symbol;
            loop {
                match state.analysis.parent_symbol_map.get(current) {
                    None => break,
                    Some(parent) => {
                        qualified_name.push(parent);
                        current = parent;
                    }
                }
            }

            qualified_name.reverse();
            let qualified_idents: Vec<&str> = qualified_name
                .into_iter()
                .map(|n| n.name().data.as_str())
                .collect();

            qualified_idents.join(".")
        } else {
            hover_node.data.clone()
        }
    };

    let symbol_kind_line = format!("({symbol_kind}) {qual_ident}");

    let markdown_lines: Vec<String> = node_data
        .pre_annotation
        .clone()
        .into_iter()
        .chain(vec!["".to_string(), symbol_kind_line, "".to_string()])
        .chain(node_data.post_annotation.clone())
        .collect();

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: markdown_lines.join("\n").trim().to_string(),
        }),
        range: Some(node_to_range(state, hover_node.id())),
    })
}

fn formal_param_to_string(state: &GlobalState, param: &FormalParam) -> String {
    let kind_s = match param.kind {
        FormalParamKind::Ref => "ref ",
        FormalParamKind::Value => "",
    };

    format!(
        "{kind_s}{}: {}",
        param.name.data,
        state
            .analysis
            .type_map
            .get(&param.type_name.node_id)
            .map_or_else(|| "???".to_string(), |ty| ty.to_string())
    )
}

pub fn symbol_to_completion_item(state: &GlobalState, symbol: &Symbol) -> CompletionItem {
    let symbol_kind = symbol_kind_name(symbol);
    let description = {
        // The symbol may come from a stale analysis snapshot whose backing node
        // was garbage collected by an intervening `Task::Reprocess` (analysis is
        // debounced, so a completion request can race ahead of a fresh
        // snapshot). Degrade to no documentation rather than panicking on a
        // freed node handle.
        match state.context.node_try_get(&symbol.node()) {
            Some(node) if !node.pre_annotation.is_empty() => Some(node.pre_annotation.join(" ")),
            _ => None,
        }
    };

    let kind = match symbol {
        Symbol::AbsType(_) => CompletionItemKind::CLASS,
        Symbol::AliasType(_) => CompletionItemKind::CLASS,
        Symbol::ArrayType(_) => CompletionItemKind::CLASS,
        Symbol::Component(_) => CompletionItemKind::CLASS,
        Symbol::ComponentInstance(_) => CompletionItemKind::VARIABLE,
        Symbol::Constant(_) => CompletionItemKind::CONSTANT,
        Symbol::EnumType(_) => CompletionItemKind::ENUM,
        Symbol::EnumConstant(_) => CompletionItemKind::ENUM_MEMBER,
        Symbol::Interface(_) => CompletionItemKind::INTERFACE,
        Symbol::Module(_) => CompletionItemKind::MODULE,
        Symbol::Port(_) => CompletionItemKind::CLASS,
        Symbol::StateMachine(_) => CompletionItemKind::CLASS,
        Symbol::StructType(_) => CompletionItemKind::STRUCT,
        Symbol::System(_) => CompletionItemKind::CLASS,
        Symbol::Topology(_) => CompletionItemKind::CLASS,
    };

    let detail = match symbol {
        Symbol::StructType(_)
        | Symbol::AbsType(_)
        | Symbol::AliasType(_)
        | Symbol::ArrayType(_)
        | Symbol::EnumType(_) => state
            .analysis
            .type_map
            .get(&symbol.node())
            .map(Type::underlying_type)
            .map(|ty| format!(" = {ty}")),
        Symbol::Port(port) => {
            let arg_fmt: Vec<String> = port
                .params
                .iter()
                .map(|prm| formal_param_to_string(state, prm))
                .collect();

            Some(format!("({})", arg_fmt.join(", ")))
        }
        Symbol::EnumConstant(_) | Symbol::Constant(_) => state
            .analysis
            .value_map
            .get(&symbol.node())
            .map(|value| format!(" = {value}")),

        // TODO(tumbar) Add some nice details about components and component instances
        Symbol::ComponentInstance(_) => None,
        Symbol::Component(_) => None,
        Symbol::Interface(_) => None,
        Symbol::Module(_) => None,
        Symbol::StateMachine(_) => None,
        Symbol::System(_) => None,
        Symbol::Topology(_) => None,
    };

    CompletionItem {
        label: symbol.name().data.clone(),
        kind: Some(kind),
        label_details: Some(CompletionItemLabelDetails {
            detail,
            description: None,
        }),
        detail: Some(symbol_kind.to_string()),
        documentation: description.map(|d| {
            Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: d,
            })
        }),
        ..Default::default()
    }
}

pub(crate) struct GetScopeVisitor<'a> {
    source_file: SourceFile,
    looking_for: BytePos,
    context: &'a CompilerContext<LspDiagnosticsEmitter>,
}

impl<'ast> Visitor<'ast> for GetScopeVisitor<'ast> {
    type Break = Vec<Node<'ast>>;
    type State = ();

    /// The default node visiting before.
    /// By default, this will just continue without visiting the children of `node`
    fn super_visit(&self, a: &mut Self::State, node: Node<'ast>) -> ControlFlow<Self::Break> {
        let span = self
            .context
            .span_get(&self.context.node_get_span(&node.id()));

        let src_file: SourceFile = span.file.upgrade().unwrap().as_ref().into();

        match node {
            // Build up scopes for nodes that can have scopes
            Node::DefStateMachine(_)
            | Node::DefModule(_)
            | Node::DefComponent(_)
            | Node::DefEnum(_) => match node.walk(a, self) {
                ControlFlow::Continue(_) => {
                    if src_file == self.source_file
                        && span.start <= self.looking_for
                        && span.start + span.length >= self.looking_for
                    {
                        ControlFlow::Break(vec![node])
                    } else {
                        ControlFlow::Continue(())
                    }
                }
                ControlFlow::Break(mut sub) => {
                    sub.push(node);
                    ControlFlow::Break(sub)
                }
            },
            _ => {
                if src_file == self.source_file {
                    // Check if this node spans the range we are looking for
                    if span.start <= self.looking_for
                        && span.start + span.length >= self.looking_for
                    {
                        // We have reached a part of the AST that surrounds the position
                        // We are the deepest we can be in the scope list
                        ControlFlow::Break(vec![])
                    } else {
                        // This node does not span the range
                        // We don't need to walk it since it's children won't span it either
                        ControlFlow::Continue(())
                    }
                } else {
                    // The files don't match
                    // We could be looking for something inside an include
                    // Keep recursing
                    node.walk(a, self)
                }
            }
        }
    }
}

pub fn scope_at_offset<'a>(
    state: &'a GlobalState,
    document: &Uri,
    offset: BytePos,
) -> Option<Vec<Node<'a>>> {
    let files = state.files.get(document.as_str())?;

    files.first().and_then(|file| {
        let cache = state.cache.get(&state.parent_file(*file)).unwrap();

        let visitor = GetScopeVisitor {
            source_file: *file,
            looking_for: offset,
            context: &state.context,
        };

        visitor.visit_trans_unit(&mut (), &cache.ast).break_value()
    })
}

pub fn completion_items_in_name_group(
    state: &GlobalState,
    cursor_pos: TextSize,
    ng: NameGroup,
    uri: &Uri,
) -> Option<Vec<CompletionItem>> {
    // If this is first token which means we need to list the first level of all valid
    // symbols. Query the analysis to extract the current scope of the cursor.

    let current_scope: Vec<String> = scope_at_offset(state, uri, cursor_pos.into())
        .unwrap_or(vec![])
        .into_iter()
        .rev()
        .map(|n| match n {
            Node::DefComponent(n) => n.name.data.clone(),
            Node::DefEnum(n) => n.name.data.clone(),
            Node::DefModule(n) => n.name.data.clone(),
            Node::DefStateMachine(n) => n.name.data.clone(),
            _ => unreachable!(),
        })
        .collect();

    // Merge all symbols going up from each scope
    let items: Vec<Vec<CompletionItem>> = current_scope
        .iter()
        .fold(
            (
                vec![
                    state
                        .analysis
                        .global_scope
                        .get_group(ng)
                        .iter()
                        .map(|(_, s)| symbol_to_completion_item(state, s))
                        .collect(),
                ],
                Some(&state.analysis.global_scope),
            ),
            |(mut out, scope), scope_name| {
                if let Some(scope) = scope {
                    let new_scope = scope
                        .get(ng, scope_name)
                        .and_then(|symbol| state.analysis.symbol_scope_map.get(&symbol));

                    match new_scope {
                        None => {}
                        Some(s) => {
                            out.push(
                                s.get_group(ng)
                                    .iter()
                                    .map(|(_, s)| symbol_to_completion_item(state, s))
                                    .collect(),
                            );
                        }
                    }

                    (out, new_scope)
                } else {
                    (out, None)
                }
            },
        )
        .0;

    // The closest symbols should appear first
    // Flip the completion items and flatten everything
    Some(items.into_iter().rev().flatten().collect())
}

/// Complete member selection on a value expression such as `a.b.c.`.
///
/// The `postfix_expr` node covers the whole `EXPR_POSTFIX` up to (and
/// including) the trailing `.` at the cursor. The receiver — everything
/// before that dot — can resolve two different ways:
///
/// 1. A qualifier symbol (module / enum) reachable through the use-def map.
///    In that case we complete the symbols in its scope's [`NameGroup::Value`]
///    group (nested constants, enum constants, ...).
/// 2. A struct-typed value. In that case the expression has an entry in the
///    type map and we complete its struct member names.
pub fn completion_items_for_postfix_expr(
    state: &GlobalState,
    postfix_expr: &SyntaxNode,
    cursor_pos: TextSize,
    uri: &Uri,
) -> Option<Vec<CompletionItem>> {
    // Collect the identifier tokens making up the receiver expression
    // (everything before the trailing dot at the cursor).
    let tokens: Vec<SyntaxToken> = postfix_expr
        .descendants_with_tokens()
        .filter_map(|s| s.as_token().cloned())
        .filter(|t| t.kind() == SyntaxKind::IDENT && t.text_range().end() <= cursor_pos)
        .collect();

    // The receiver must end in an identifier for us to resolve it.
    let last_token_pos = tokens.last()?.text_range().start().into();

    // Case 1: the receiver resolves to a symbol that owns a scope
    // (e.g. `Svc.Fpy` -> a module, or an enum used as a qualifier).
    if let Some(items) = symbol_at_position(state, uri, last_token_pos)
        .and_then(|(_, symbol)| state.analysis.symbol_scope_map.get(&symbol))
        .map(|scope| {
            scope
                .get_group(NameGroup::Value)
                .iter()
                .map(|(_, child_symbol)| symbol_to_completion_item(state, child_symbol))
                .collect::<Vec<_>>()
        })
    {
        return Some(items);
    }

    // Case 2: the receiver is a struct-typed value expression. Find the AST
    // expression node covering the receiver and complete its struct members.
    let nodes = nodes_at_offset(state, uri, last_token_pos)?;
    let receiver_ty = nodes
        .iter()
        .find_map(|node| state.analysis.type_map.get(&node.id()))?;

    let underlying = Type::underlying_type(receiver_ty);
    let members = match underlying.deref() {
        Type::Struct(struct_ty) => &struct_ty.anon_struct.members,
        Type::AnonStruct(anon_struct) => &anon_struct.members,
        _ => return None,
    };

    let mut items: Vec<CompletionItem> = members
        .iter()
        .map(|(name, member_ty)| CompletionItem {
            label: name.clone(),
            kind: Some(CompletionItemKind::FIELD),
            label_details: Some(CompletionItemLabelDetails {
                detail: Some(format!(": {}", member_ty)),
                description: None,
            }),
            ..Default::default()
        })
        .collect();
    items.sort_by(|a, b| a.label.cmp(&b.label));
    Some(items)
}

/// Resolve an interface-instance symbol to its port interface.
///
/// An interface instance is either a component instance (whose ports come from
/// its component's port interface) or an imported (sub)topology (whose ports
/// come from the topology's own port interface). This is the generic lookup
/// shared by connection endpoints and topology port aliases.
fn port_interface_for_instance_symbol<'a>(
    state: &'a GlobalState,
    symbol: &Symbol,
) -> Option<&'a PortInterface> {
    match symbol {
        Symbol::ComponentInstance(_) => {
            let instance = state.analysis.component_instance_map.get(symbol)?;
            let component = state
                .analysis
                .component_map
                .get(&instance.component_symbol)?;
            Some(&component.port_interface)
        }
        Symbol::Topology(_) => Some(&state.analysis.topology_map.get(symbol)?.port_interface),
        _ => None,
    }
}

/// Format the signature of the port definition backing a port instance, as a
/// function-call-style string: `(param: Type, ...): ReturnType`.
///
/// Returns `None` for serial ports and instances with no underlying port
/// definition (e.g. internal ports), which have no parameter signature.
fn port_signature(state: &GlobalState, pi: &PortInstance) -> Option<String> {
    let def = match pi.get_type()? {
        PortInstanceType::DefPort(symbol) => symbol,
        PortInstanceType::Serial => return None,
    };

    let args: Vec<String> = def
        .params
        .iter()
        .map(|prm| formal_param_to_string(state, prm))
        .collect();

    let return_ty = def.return_type.as_ref().and_then(|tn| {
        state
            .analysis
            .type_map
            .get(&tn.node_id)
            .map(|ty| format!(": {ty}"))
    });

    Some(format!(
        "({}){}",
        args.join(", "),
        return_ty.unwrap_or_default()
    ))
}

/// Resolve a port-instance identifier's port name at `position` to its
/// underlying [`PortInstance`].
///
/// Port names in connections (`instance.portName`) resolve to a `PortInstance`,
/// not a `Symbol`, so they are absent from `use_def_map` and are not handled by
/// the generic hover/goto paths. This re-runs the same resolution that
/// [`fpp_analysis::semantics::PortInstanceIdentifier::from_node`] uses: the
/// instance qualifier is already in `use_def_map`, so we resolve it via
/// [`Analysis::get_interface_instance`] and then look up the port by name.
///
/// Returns the port name's AST node (for locating/ranging the hover) together
/// with the resolved port instance. Returns `None` if the cursor is not on a
/// port name, or the instance/port does not resolve.
pub(crate) fn port_instance_at_position<'a>(
    state: &'a GlobalState,
    document: &Uri,
    position: BytePos,
) -> Option<(Node<'a>, PortInstance)> {
    let nodes = nodes_at_offset(state, document, position)?;

    // Find the enclosing port-instance identifier and confirm the cursor is on
    // the port name (not the instance qualifier).
    let pii = nodes.iter().find_map(|n| match n {
        Node::PortInstanceIdentifier(pii) => Some(*pii),
        _ => None,
    })?;

    let name_span = state
        .context
        .span_get(&state.context.node_get_span(&pii.port_name.id()));
    if position < name_span.start || position > name_span.start + name_span.length {
        return None;
    }

    // Resolve the instance qualifier to an interface instance, then look up the
    // port by name. This reuses the analysis resolution helpers, which are
    // usable here because the request runs under `fpp_core::run_ref` (the
    // compiler context is set on this thread).
    let interface_instance = state
        .analysis
        .get_interface_instance(pii.interface_instance.id())?;
    let port_instance = interface_instance
        .get_port_instance(&state.analysis, &pii.port_name)
        .ok()?;

    // Look up the port name node among the resolved nodes for ranging.
    let name_node = nodes
        .iter()
        .find(|n| n.id() == pii.port_name.id())
        .copied()
        .unwrap_or(Node::PortInstanceIdentifier(pii));

    Some((name_node, port_instance))
}

/// Build hover information for a resolved port instance, mirroring the
/// signature + annotation format used by the port completion items.
pub fn hover_for_port_instance(state: &GlobalState, hover_node: Node, pi: &PortInstance) -> Hover {
    let direction = Direction::show(&pi.get_direction());
    let port_ty = PortInstanceType::show(&pi.get_type());
    let signature = port_signature(state, pi).unwrap_or_default();

    let node_data = state.context.node_get(&pi.get_node_id());
    let kind_line = format!(
        "({direction} port) {}: {port_ty}{signature}",
        pi.get_unqualified_name()
    );

    let markdown_lines: Vec<String> = node_data
        .pre_annotation
        .clone()
        .into_iter()
        .chain(vec!["".to_string(), kind_line, "".to_string()])
        .chain(node_data.post_annotation.clone())
        .collect();

    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: markdown_lines.join("\n").trim().to_string(),
        }),
        range: Some(node_to_range(state, hover_node.id())),
    }
}

/// Resolve a port-match specifier's port name at `position` to its underlying
/// [`PortInstance`].
///
/// Port match specifiers (`match portA with portB`) name their two ports with
/// bare [`fpp_ast::Ident`]s (raw string + span), not symbol references, so they
/// are absent from `use_def_map` and are not handled by the generic hover/goto
/// paths. Resolution mirrors `construct_port_matching` in `fpp_analysis`: look
/// the name up in the enclosing component's `port_interface.port_map`.
///
/// Returns the port name's AST node (for locating/ranging the hover) together
/// with the resolved port instance. Returns `None` if the cursor is not on one
/// of the two port names, or the component/port does not resolve.
pub(crate) fn port_match_at_position<'a>(
    state: &'a GlobalState,
    document: &Uri,
    position: BytePos,
) -> Option<(Node<'a>, PortInstance)> {
    let nodes = nodes_at_offset(state, document, position)?;

    // Find the enclosing port-match specifier and confirm the cursor is on one
    // of its two port names.
    let spec = nodes.iter().find_map(|n| match n {
        Node::SpecPortMatching(spec) => Some(*spec),
        _ => None,
    })?;

    let on_name = |name: &fpp_ast::Ident| -> bool {
        let span = state
            .context
            .span_get(&state.context.node_get_span(&name.id()));
        position >= span.start && position <= span.start + span.length
    };
    let name = if on_name(&spec.port1) {
        &spec.port1
    } else if on_name(&spec.port2) {
        &spec.port2
    } else {
        return None;
    };

    // Resolve the enclosing component and look the port up by name, reusing the
    // same lookup `construct_port_matching` performs. This is usable here
    // because the request runs under `fpp_core::run_ref` (the compiler context
    // is set on this thread).
    let component = enclosing_component(state, &nodes)?;
    let port_instance = component.port_map().get(&name.data)?.clone();

    // Look up the port name node among the resolved nodes for ranging.
    let name_node = nodes
        .iter()
        .find(|n| n.id() == name.id())
        .copied()
        .unwrap_or(Node::SpecPortMatching(spec));

    Some((name_node, port_instance))
}

/// The resolved component enclosing the cursor, if any, given the AST nodes
/// covering the cursor position (innermost first).
fn enclosing_component<'a>(state: &'a GlobalState, nodes: &[Node<'_>]) -> Option<&'a Component> {
    let component_node = nodes.iter().find_map(|n| match n {
        Node::DefComponent(def) => Some(*def),
        _ => None,
    })?;
    let symbol = state.analysis.symbol_map.get(&component_node.node_id)?;
    state.analysis.component_map.get(symbol)
}

/// Completion items for a port name in a port-match specifier (`match a with b`).
///
/// Both names refer to general port instances of the enclosing component. Per
/// the matched-numbering convention the first name is an output port and the
/// second an input port, so we filter by `direction`. The enclosing component
/// is resolved through the semantic AST (via [`enclosing_component`]), which is
/// robust to the specifier being incomplete in the lossless parse.
pub(crate) fn completion_items_for_port_match(
    state: &GlobalState,
    document: &Uri,
    position: BytePos,
    direction: Direction,
) -> Option<Vec<CompletionItem>> {
    let nodes = nodes_at_offset(state, document, position)?;
    let component = enclosing_component(state, &nodes)?;
    Some(completion_items_for_ports(
        state,
        &component.port_interface,
        Some(direction),
        true,
    ))
}

/// Resolve a use inside a state machine at `position` to its state machine
/// symbol (action, guard, signal, state, or choice).
///
/// Uses inside a state machine are recorded in that state machine's own
/// `use_def_map` (keyed by node id), separate from the global `use_def_map`, so
/// hover/goto must resolve them here. Returns the resolved use node together
/// with its symbol.
pub(crate) fn sm_symbol_at_position<'a>(
    state: &'a GlobalState,
    document: &Uri,
    position: BytePos,
) -> Option<(Node<'a>, &'a StateMachine, StateMachineSymbol)> {
    let nodes = nodes_at_offset(state, document, position)?;

    // Find the enclosing state machine definition and its analysis.
    let state_machine = enclosing_state_machine(state, &nodes)?;

    // Find the deepest node at the cursor that is a use in this state machine.
    nodes.iter().find_map(|n| {
        state_machine
            .sma
            .use_def_map
            .get(&n.id())
            .map(|sym| (*n, state_machine, sym.clone()))
    })
}

/// Resolve a state machine member *definition* at `position` to its state
/// machine symbol (action, guard, signal, state, or choice).
///
/// State machine members (actions, guards, signals, states, choices) are not
/// entered into the global `symbol_map`; they live only in their state
/// machine's own analysis. So hovering the name of such a definition would
/// otherwise fall through to the enclosing `DefStateMachine`, showing the
/// parent state machine's hover instead of the member's. This resolves the
/// member definition node the cursor is on to a [`StateMachineSymbol`] built
/// from that node, mirroring how uses resolve via [`sm_symbol_at_position`].
///
/// The returned node is the member definition's own node, used for the hover
/// range.
pub(crate) fn sm_def_at_position<'a>(
    state: &'a GlobalState,
    document: &Uri,
    position: BytePos,
) -> Option<(Node<'a>, &'a StateMachine, StateMachineSymbol)> {
    let nodes = nodes_at_offset(state, document, position)?;

    // A definition-name hover only applies when the cursor is on the name.
    if !matches!(nodes.first(), Some(Node::Name(_))) {
        return None;
    }

    // Find the enclosing state machine definition and its analysis.
    let state_machine = enclosing_state_machine(state, &nodes)?;

    // Find the deepest state machine member definition node at the cursor and
    // build its symbol.
    nodes.iter().find_map(|n| {
        let symbol = match n {
            Node::DefAction(def) => StateMachineSymbol::Action(Arc::new((*def).clone())),
            Node::DefGuard(def) => StateMachineSymbol::Guard(Arc::new((*def).clone())),
            Node::DefSignal(def) => StateMachineSymbol::Signal(Arc::new((*def).clone())),
            Node::DefState(def) => StateMachineSymbol::State(Arc::new((*def).clone())),
            Node::DefChoice(def) => StateMachineSymbol::Choice(Arc::new((*def).clone())),
            _ => return None,
        };
        Some((*n, state_machine, symbol))
    })
}

/// The resolved state machine enclosing the cursor, if any, given the AST nodes
/// covering the cursor position (innermost first).
fn enclosing_state_machine<'a>(
    state: &'a GlobalState,
    nodes: &[Node<'_>],
) -> Option<&'a StateMachine> {
    let sm_node = nodes.iter().find_map(|n| match n {
        Node::DefStateMachine(def) => Some(*def),
        _ => None,
    })?;
    let sm_symbol = state.analysis.symbol_map.get(&sm_node.node_id)?;
    state.analysis.state_machine_map.get(sm_symbol)
}

/// A state machine completion target, selecting which kind of definitions to
/// offer at the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SmCompletionKind {
    /// Action names, as in `do { <here> }`.
    Action,
    /// Guard names, as in `if <here>`.
    Guard,
    /// Signal names, as in `on <here>`.
    Signal,
    /// State and choice names, as in `enter <here>`.
    StateOrChoice,
}

/// Build completion items for state machine definitions of the requested kind,
/// resolved against the state machine enclosing `position`.
///
/// Actions, guards, and signals are top-level state machine definitions.
/// States and choices may be nested; they are offered by their fully qualified
/// dotted path (e.g. `S2.S3`), matching how `enter` targets are written.
pub(crate) fn completion_items_for_sm(
    state: &GlobalState,
    document: &Uri,
    position: BytePos,
    kind: SmCompletionKind,
) -> Option<Vec<CompletionItem>> {
    let nodes = nodes_at_offset(state, document, position)?;
    let sm = enclosing_state_machine(state, &nodes)?;

    let items = match kind {
        SmCompletionKind::Action => sm
            .actions
            .iter()
            .map(|s| sm_symbol_to_completion_item(state, s))
            .collect(),
        SmCompletionKind::Guard => sm
            .guards
            .iter()
            .map(|s| sm_symbol_to_completion_item(state, s))
            .collect(),
        SmCompletionKind::Signal => sm
            .signals
            .iter()
            .map(|s| sm_symbol_to_completion_item(state, s))
            .collect(),
        SmCompletionKind::StateOrChoice => {
            // Completed level-by-level (see `completion_items_for_sm_state`).
            return completion_items_for_sm_state(state, document, position, &[]);
        }
    };
    Some(items)
}

/// Complete state/choice names for an `enter` transition target.
///
/// `qualifier` is the sequence of state names already typed before the final
/// dot. Two cases:
///
/// * **Empty qualifier** (first, unqualified segment): FPP resolves an
///   unqualified target through the nested scope chain — the enclosing state,
///   then each ancestor state, then the state machine root. We offer the direct
///   states/choices at every level of that chain, so a target defined in a
///   parent state (e.g. `enter PARENT_CHOICE` from within a substate) is
///   included. Names shadowed by an inner scope are offered once (innermost
///   wins), matching resolution.
/// * **Non-empty qualifier** (after a dot, `enter A.B.`): explicit navigation,
///   so we offer only the resolved qualifier state's direct children. The editor
///   re-triggers on `.` to descend.
pub(crate) fn completion_items_for_sm_state(
    state: &GlobalState,
    document: &Uri,
    position: BytePos,
    qualifier: &[String],
) -> Option<Vec<CompletionItem>> {
    use fpp_ast::{StateMachineMember, StateMember};

    let nodes = nodes_at_offset(state, document, position)?;
    let sm = enclosing_state_machine(state, &nodes)?;

    let state_item = |st: &fpp_ast::DefState| {
        sm_symbol_to_completion_item(state, &StateMachineSymbol::State(Arc::new(st.clone())))
    };
    let choice_item = |ch: &fpp_ast::DefChoice| {
        sm_symbol_to_completion_item(state, &StateMachineSymbol::Choice(Arc::new(ch.clone())))
    };

    let mut items = Vec::new();

    if qualifier.is_empty() {
        // Offer the direct states/choices of the enclosing scope chain:
        // innermost enclosing state outward to each ancestor state, then the
        // state machine root. `nodes_at_offset` yields the enclosing `DefState`s
        // innermost-first, which is exactly the resolution order.
        let mut seen = std::collections::HashSet::new();
        let mut push_state_members = |members: &[StateMember], items: &mut Vec<CompletionItem>| {
            for member in members {
                match member {
                    StateMember::DefState(st) if seen.insert(st.name.data.clone()) => {
                        items.push(state_item(st))
                    }
                    StateMember::DefChoice(ch) if seen.insert(ch.name.data.clone()) => {
                        items.push(choice_item(ch))
                    }
                    _ => {}
                }
            }
        };
        for node in &nodes {
            if let Node::DefState(st) = node {
                push_state_members(&st.members, &mut items);
            }
        }
        // Finally the state machine root's own states/choices.
        for member in sm.node.members.iter().flatten() {
            match member {
                StateMachineMember::DefState(st) if seen.insert(st.name.data.clone()) => {
                    items.push(state_item(st))
                }
                StateMachineMember::DefChoice(ch) if seen.insert(ch.name.data.clone()) => {
                    items.push(choice_item(ch))
                }
                _ => {}
            }
        }
    } else {
        // Resolve the qualifier path to the state whose direct children to offer.
        let mut current: Option<&fpp_ast::DefState> = None;
        for (i, seg) in qualifier.iter().enumerate() {
            let next = if i == 0 {
                sm.node.members.iter().flatten().find_map(|m| match m {
                    StateMachineMember::DefState(st) if st.name.data == *seg => Some(st),
                    _ => None,
                })
            } else {
                current?.members.iter().find_map(|m| match m {
                    StateMember::DefState(st) if st.name.data == *seg => Some(st),
                    _ => None,
                })
            };
            current = Some(next?);
        }
        for member in &current?.members {
            match member {
                StateMember::DefState(st) => items.push(state_item(st)),
                StateMember::DefChoice(ch) => items.push(choice_item(ch)),
                _ => {}
            }
        }
    }

    items.sort_by(|a, b| a.label.cmp(&b.label));
    Some(items)
}

/// Build a completion item for a state machine symbol, labeled by its
/// unqualified name. Nested states/choices are completed one level at a time, so
/// no qualified path is needed on the label.
fn sm_symbol_to_completion_item(
    state: &GlobalState,
    symbol: &StateMachineSymbol,
) -> CompletionItem {
    let (kind, detail) = match symbol {
        StateMachineSymbol::Action(_) => (CompletionItemKind::FUNCTION, "action"),
        StateMachineSymbol::Guard(_) => (CompletionItemKind::VARIABLE, "guard"),
        StateMachineSymbol::Signal(_) => (CompletionItemKind::EVENT, "signal"),
        StateMachineSymbol::State(_) => (CompletionItemKind::VARIABLE, "state"),
        StateMachineSymbol::Choice(_) => (CompletionItemKind::VARIABLE, "choice"),
    };

    let node = state.context.node_get(&symbol.node());
    let documentation = if node.pre_annotation.is_empty() {
        None
    } else {
        Some(Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: node.pre_annotation.join(" "),
        }))
    };

    CompletionItem {
        label: symbol.get_unqualified_name().to_string(),
        kind: Some(kind),
        detail: Some(detail.to_string()),
        documentation,
        ..Default::default()
    }
}

/// Build hover information for a resolved state machine symbol, mirroring the
/// `(kind) qualified.name` format used by [`hover_for_symbol`].
pub fn hover_for_sm_symbol(
    state: &GlobalState,
    hover_node: Node,
    state_machine: &StateMachine,
    symbol: &StateMachineSymbol,
) -> Hover {
    let kind = match symbol {
        StateMachineSymbol::Action(_) => "Action",
        StateMachineSymbol::Guard(_) => "Guard",
        StateMachineSymbol::Signal(_) => "Signal",
        StateMachineSymbol::State(_) => "State",
        StateMachineSymbol::Choice(_) => "Choice",
    };

    let node_data = state.context.node_get(&symbol.node());
    let sm_name = state_machine.node.name.data.as_str();
    let qualified_name = format!("{sm_name}.{}", state_machine.sma.get_qualified_name(symbol));
    let kind_line = format!("({kind}) {qualified_name}");

    let markdown_lines: Vec<String> = node_data
        .pre_annotation
        .clone()
        .into_iter()
        .chain(vec!["".to_string(), kind_line, "".to_string()])
        .chain(node_data.post_annotation.clone())
        .collect();

    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: markdown_lines.join("\n").trim().to_string(),
        }),
        range: Some(node_to_range(state, hover_node.id())),
    }
}

/// Build completion items for the ports of a port interface.
///
/// The `port_map` holds every port instance (general, special, and internal),
/// which is the set of names valid after `instance.` in a connection endpoint
/// or topology port alias.
///
/// Each item is formatted like a function-call lookup: the label detail carries
/// the port's `(params): ReturnType` signature (from the underlying port
/// definition) alongside its direction and port type, and the port instance's
/// annotation is shown as documentation.
///
/// `direction_filter` restricts the results to ports of a given direction: the
/// `from` side of a connection accepts only outputs and the `to` side only
/// inputs. `None` keeps every port (used for topology port aliases, which may
/// alias either direction). Internal ports have no direction, so any directional
/// filter also (correctly) excludes them — they cannot be connected.
fn completion_items_for_ports(
    state: &GlobalState,
    port_interface: &PortInterface,
    direction_filter: Option<Direction>,
    general_only: bool,
) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = port_interface
        .port_map
        .values()
        .filter(|pi| !general_only || matches!(pi, PortInstance::General(_)))
        .filter(|pi| match direction_filter {
            Some(want) => pi.get_direction() == Some(want),
            None => true,
        })
        .map(|pi| {
            let direction = Direction::show(&pi.get_direction());
            let port_ty = PortInstanceType::show(&pi.get_type());
            let signature = port_signature(state, pi);

            // The annotation on the port instance definition, if any.
            let annotation = {
                let node = state.context.node_get(&pi.get_node_id());
                if node.pre_annotation.is_empty() {
                    None
                } else {
                    Some(node.pre_annotation.join(" "))
                }
            };

            CompletionItem {
                label: pi.get_unqualified_name().to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                label_details: Some(CompletionItemLabelDetails {
                    // Function-call-style signature next to the label.
                    detail: signature.clone(),
                    // The port type shown on the right, e.g. `Drv.DataReturn`.
                    description: Some(port_ty.clone()),
                }),
                detail: Some(format!(
                    "{direction} port {port_ty}{}",
                    signature.unwrap_or_default()
                )),
                documentation: annotation.map(|a| {
                    Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: a,
                    })
                }),
                ..Default::default()
            }
        })
        .collect();
    items.sort_by(|a, b| a.label.cmp(&b.label));
    items
}

/// Resolve a dotted name (given as its segment strings) to a symbol by walking
/// the lexical scope chain at `cursor_pos`, in the given name group.
///
/// Unlike [`symbol_at_position`], this does not depend on `use_def_map`, so it
/// works even when the surrounding construct fails to parse in the semantic
/// parser (e.g. an incomplete `instance.` connection endpoint). It mirrors the
/// scope-chain construction in [`completion_items_in_name_group`].
fn resolve_symbol_in_scope_chain(
    state: &GlobalState,
    segments: &[String],
    ng: NameGroup,
    cursor_pos: TextSize,
    uri: &Uri,
) -> Option<Symbol> {
    let (first, rest) = segments.split_first()?;

    // Build the stack of enclosing scopes from outermost (global) to innermost.
    let enclosing_names: Vec<String> = scope_at_offset(state, uri, cursor_pos.into())
        .unwrap_or_default()
        .into_iter()
        .rev()
        .map(|n| match n {
            Node::DefComponent(n) => n.name.data.clone(),
            Node::DefEnum(n) => n.name.data.clone(),
            Node::DefModule(n) => n.name.data.clone(),
            Node::DefStateMachine(n) => n.name.data.clone(),
            _ => unreachable!(),
        })
        .collect();

    let mut scopes: Vec<&Scope> = vec![&state.analysis.global_scope];
    for name in &enclosing_names {
        match scopes
            .last()
            .and_then(|s| s.get(ng, name))
            .and_then(|sym| state.analysis.symbol_scope_map.get(&sym))
        {
            Some(scope) => scopes.push(scope),
            None => break,
        }
    }

    // Resolve the first segment by searching innermost scope outward.
    let mut symbol = scopes.iter().rev().find_map(|scope| scope.get(ng, first))?;

    // Descend into each subsequent qualifier segment.
    for seg in rest {
        let scope = state.analysis.symbol_scope_map.get(&symbol)?;
        symbol = scope.get(ng, seg)?;
    }

    Some(symbol)
}

/// Complete a port-instance identifier (`instance.PortName`) as used in
/// topology connection endpoints and topology port aliases.
///
/// `receiver_tokens` are the `IDENT` tokens of the qualifier before the final
/// dot. `after_dot` indicates whether the cursor follows a `.` (so we complete
/// port names / nested instances) versus sitting on the first bare segment (so
/// we complete interface-instance names). `direction_filter` restricts port
/// results by direction: outputs on the `from` side of `->`, inputs on the `to`
/// side, and `None` (unfiltered) for topology port aliases.
pub fn completion_items_for_port_instance(
    state: &GlobalState,
    receiver_tokens: &[SyntaxToken],
    after_dot: bool,
    direction_filter: Option<Direction>,
    cursor_pos: TextSize,
    uri: &Uri,
) -> Option<Vec<CompletionItem>> {
    if !after_dot {
        // First segment: complete the available interface-instance names.
        return completion_items_in_name_group(
            state,
            cursor_pos,
            NameGroup::PortInterfaceInstance,
            uri,
        );
    }

    // After a dot: resolve the receiver through the scope chain. We cannot use
    // `use_def_map` here because an incomplete `instance.` endpoint fails to
    // parse in the semantic parser, so the receiver has no use-def entry.
    let segments: Vec<String> = receiver_tokens
        .iter()
        .map(|t| t.text().to_string())
        .collect();

    let symbol = resolve_symbol_in_scope_chain(
        state,
        &segments,
        NameGroup::PortInterfaceInstance,
        cursor_pos,
        uri,
    )?;

    // If the receiver is an interface instance, complete its ports.
    if let Some(port_interface) = port_interface_for_instance_symbol(state, &symbol) {
        return Some(completion_items_for_ports(
            state,
            port_interface,
            direction_filter,
            false,
        ));
    }

    // Otherwise the receiver is a qualifier still being typed (e.g. a
    // subtopology / module namespace); complete the interface instances in its
    // scope.
    state.analysis.symbol_scope_map.get(&symbol).map(|scope| {
        scope
            .get_group(NameGroup::PortInterfaceInstance)
            .iter()
            .map(|(_, child_symbol)| symbol_to_completion_item(state, child_symbol))
            .collect()
    })
}

pub fn completion_items_for_qual_ident(
    state: &GlobalState,
    qual_ident: SyntaxElement,
    cursor_pos: TextSize,
    ng: NameGroup,
    uri: &Uri,
) -> Option<Vec<CompletionItem>> {
    let tokens: Vec<SyntaxToken> = qual_ident
        .as_node()
        .map(|node| {
            node.descendants_with_tokens()
                .filter_map(|s| s.as_token().cloned())
                .filter(|t| t.kind() == SyntaxKind::IDENT && t.text_range().end() <= cursor_pos)
                .collect()
        })
        .unwrap_or(vec![]);

    if tokens.is_empty() {
        completion_items_in_name_group(state, cursor_pos, ng, uri)
    } else {
        // Get the final token before the cursor to look it up in the AST
        let last_token_pos = tokens.last().unwrap().text_range().start().into();

        // Look up the symbol before the cursor
        symbol_at_position(state, uri, last_token_pos)
            .and_then(|(_, symbol)| state.analysis.symbol_scope_map.get(&symbol))
            .map(|scope| {
                // Get all symbols under this symbol's scope in the proper
                // name group
                scope
                    .get_group(ng)
                    .iter()
                    .map(|(_, child_symbol)| symbol_to_completion_item(state, child_symbol))
                    .collect()
            })
    }
}
