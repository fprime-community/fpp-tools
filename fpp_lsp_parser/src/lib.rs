mod event;
mod grammar;
mod input;
mod lexed_str;
mod output;
mod parser;
mod ptr;
mod shortcuts;
mod syntax;
mod syntax_error;
mod syntax_node;
mod token_set;
mod token_text;
mod visitor;

pub use input::*;
pub use lexed_str::*;
pub use output::*;
pub use shortcuts::*;
use std::sync::Arc;
pub use syntax::*;
pub use visitor::*;

pub use crate::{
    ptr::{AstPtr, SyntaxNodePtr},
    syntax_error::SyntaxError,
    syntax_node::{
        FppLanguage, PreorderWithTokens, SyntaxElement, SyntaxElementChildren, SyntaxNode,
        SyntaxNodeChildren, SyntaxToken, SyntaxTreeBuilder,
    },
    token_text::TokenText,
};

/// Parse the whole of the input as a given syntactic construct.
///
/// This covers two main use-cases:
///
///   * Parsing an FPP file.
///   * Parsing an `.fppi` file.
///
/// Normal `.fpp` files have `Module` entrypoint. The other entry-points
/// come from `include` specifiers: a fragment is parsed with the members of
/// the construct that includes it.
///
///
/// [`TopEntryPoint::parse`] makes a guarantee that
///   * all input is consumed
///   * the result is a valid tree (there's one root node)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopEntryPoint {
    Module,
    Component,
    Topology,
    TlmPacket,
    TlmPacketSet,
    StateMachine,
    State,
}

impl TopEntryPoint {
    /// Parse an entrypoint from its CLI name. Accepts kebab-case, snake_case,
    /// and camelCase spellings (e.g. `tlm-packet-set`, `tlm_packet_set`,
    /// `tlmPacketSet`).
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "module" => Some(TopEntryPoint::Module),
            "component" => Some(TopEntryPoint::Component),
            "topology" => Some(TopEntryPoint::Topology),
            "tlm-packet" | "tlm_packet" | "tlmPacket" => Some(TopEntryPoint::TlmPacket),
            "tlm-packet-set" | "tlm_packet_set" | "tlmPacketSet" => {
                Some(TopEntryPoint::TlmPacketSet)
            }
            "state-machine" | "state_machine" | "stateMachine" => Some(TopEntryPoint::StateMachine),
            "state" => Some(TopEntryPoint::State),
            _ => None,
        }
    }

    pub fn parse(&self, input: &Input) -> Output {
        let entry_point: fn(&'_ mut parser::Parser<'_>) = match self {
            TopEntryPoint::Module => grammar::entry::module_entry,
            TopEntryPoint::Component => grammar::entry::component_entry,
            TopEntryPoint::Topology => grammar::entry::topology_entry,
            TopEntryPoint::TlmPacket => grammar::entry::tlm_packet_entry,
            TopEntryPoint::TlmPacketSet => grammar::entry::tlm_packet_set_entry,
            TopEntryPoint::StateMachine => grammar::entry::state_machine_entry,
            TopEntryPoint::State => grammar::entry::state_entry,
        };
        let mut p = parser::Parser::new(input);
        entry_point(&mut p);
        let events = p.finish();
        event::process(events)
    }
}

pub use rowan::{
    Direction, GreenNode, NodeOrToken, SyntaxText, TextRange, TextSize, TokenAtOffset, WalkEvent,
    api::Preorder,
};

pub(crate) fn parse_text(text: &str, entry: TopEntryPoint) -> (GreenNode, Vec<SyntaxError>) {
    let lexed = LexedStr::new(text);
    let parser_input = lexed.to_input();
    let parser_output = entry.parse(&parser_input);
    let (node, errors, _eof) = build_tree(lexed, parser_output);
    (node, errors)
}

pub(crate) fn build_tree(
    lexed: LexedStr<'_>,
    parser_output: Output,
) -> (GreenNode, Vec<SyntaxError>, bool) {
    let mut builder = SyntaxTreeBuilder::default();

    let is_eof = lexed.intersperse_trivia(&parser_output, &mut |step| match step {
        StrStep::Token { kind, text } => builder.token(kind, text),
        StrStep::Enter { kind } => builder.start_node(kind),
        StrStep::Exit => builder.finish_node(),
        StrStep::Error { msg, pos } => builder.error(msg.to_owned(), pos.try_into().unwrap()),
        StrStep::Expected { kind, start, end } => builder.expected(
            kind,
            TextRange::new(start.try_into().unwrap(), end.try_into().unwrap()),
        ),
    });

    // The lexer errors come first: the Scala parser reports them before it
    // parses anything.
    let (node, parse_errors) = builder.finish_raw();
    let mut errors: Vec<SyntaxError> = lexed
        .errors()
        .map(|(i, err)| {
            let text_range = lexed.text_range(i);
            let text_range = TextRange::new(
                text_range.start.try_into().unwrap(),
                text_range.end.try_into().unwrap(),
            );
            SyntaxError::new(err, text_range)
        })
        .collect();
    errors.extend(parse_errors);

    (node, errors, is_eof)
}

/// `Parse` is the result of the parsing: a syntax tree and a collection of
/// errors.
///
/// Note that we always produce a syntax tree, even for completely invalid
/// files.
#[derive(Debug, PartialEq, Eq)]
pub struct Parse {
    green: GreenNode,
    errors: Option<Arc<[SyntaxError]>>,
}

impl Clone for Parse {
    fn clone(&self) -> Parse {
        Parse {
            green: self.green.clone(),
            errors: self.errors.clone(),
        }
    }
}

impl Parse {
    fn new(green: GreenNode, errors: Vec<SyntaxError>) -> Parse {
        Parse {
            green,
            errors: if errors.is_empty() {
                None
            } else {
                Some(errors.into())
            },
        }
    }

    pub fn syntax_node(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn errors(&self) -> Vec<SyntaxError> {
        if let Some(e) = self.errors.as_deref() {
            e.to_vec()
        } else {
            vec![]
        }
    }
}

impl Parse {
    /// Converts this parse result into a parse result for an untyped syntax tree.
    pub fn to_syntax(self) -> Parse {
        Parse {
            green: self.green,
            errors: self.errors,
        }
    }

    /// Converts from `Parse<T>` to [`Result<T, Vec<SyntaxError>>`].
    pub fn ok(self) -> Result<SyntaxNode, Vec<SyntaxError>> {
        match self.errors() {
            errors if !errors.is_empty() => Err(errors),
            _ => Ok(self.syntax_node()),
        }
    }
}

impl Parse {
    pub fn debug_dump(&self) -> String {
        let mut buf = format!("{:#?}", self.syntax_node());
        for err in self.errors() {
            buf = buf + &format!("error {:?}: {}\n", err.range(), err);
        }
        buf
    }
}

pub fn parse(text: &str, entry: TopEntryPoint) -> Parse {
    let (green, errors) = parse_text(text, entry);
    Parse::new(green, errors)
}
