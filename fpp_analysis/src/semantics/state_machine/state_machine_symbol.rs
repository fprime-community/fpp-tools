use crate::semantics::SymbolInterface;
use fpp_core::{Node, Span, Spanned};
use std::sync::Arc;

/// A symbol that represents a definition in a state machine definition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StateMachineSymbol {
    Action(Arc<fpp_ast::DefAction>),
    Guard(Arc<fpp_ast::DefGuard>),
    Choice(Arc<fpp_ast::DefChoice>),
    Signal(Arc<fpp_ast::DefSignal>),
    State(Arc<fpp_ast::DefState>),
}

impl StateMachineSymbol {
    /// Gets the unqualified name of the symbol
    pub fn get_unqualified_name(&self) -> &str {
        self.name().data.as_str()
    }

    /// Gets the location of the definition named by this symbol
    pub fn get_span(&self) -> Span {
        match self {
            StateMachineSymbol::Action(node) => node.span(),
            StateMachineSymbol::Guard(node) => node.span(),
            StateMachineSymbol::Choice(node) => node.span(),
            StateMachineSymbol::Signal(node) => node.span(),
            StateMachineSymbol::State(node) => node.span(),
        }
    }
}

impl SymbolInterface for StateMachineSymbol {
    fn node(&self) -> Node {
        match self {
            StateMachineSymbol::Action(node) => node.node_id,
            StateMachineSymbol::Guard(node) => node.node_id,
            StateMachineSymbol::Choice(node) => node.node_id,
            StateMachineSymbol::Signal(node) => node.node_id,
            StateMachineSymbol::State(node) => node.node_id,
        }
    }

    fn name(&self) -> &fpp_ast::Name {
        match self {
            StateMachineSymbol::Action(def) => &def.name,
            StateMachineSymbol::Guard(def) => &def.name,
            StateMachineSymbol::Choice(def) => &def.name,
            StateMachineSymbol::Signal(def) => &def.name,
            StateMachineSymbol::State(def) => &def.name,
        }
    }
}
