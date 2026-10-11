use fpp_core::{Node, Span, Spanned};
use std::sync::Arc;

/// A typed element of a state machine
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StateMachineTypedElement {
    StateEntry(Arc<fpp_ast::SpecStateEntry>),
    StateExit(Arc<fpp_ast::SpecStateExit>),
    InitialTransition(Arc<fpp_ast::SpecInitialTransition>),
    StateTransition(Arc<fpp_ast::SpecStateTransition>),
    Choice(Arc<fpp_ast::DefChoice>),
}

impl StateMachineTypedElement {
    pub fn get_node_id(&self) -> Node {
        match self {
            StateMachineTypedElement::StateEntry(node) => node.node_id,
            StateMachineTypedElement::StateExit(node) => node.node_id,
            StateMachineTypedElement::InitialTransition(node) => node.node_id,
            StateMachineTypedElement::StateTransition(node) => node.node_id,
            StateMachineTypedElement::Choice(node) => node.node_id,
        }
    }

    pub fn get_span(&self) -> Span {
        match self {
            StateMachineTypedElement::StateEntry(node) => node.span(),
            StateMachineTypedElement::StateExit(node) => node.span(),
            StateMachineTypedElement::InitialTransition(node) => node.span(),
            StateMachineTypedElement::StateTransition(node) => node.span(),
            StateMachineTypedElement::Choice(node) => node.span(),
        }
    }

    pub fn show_kind(&self) -> &'static str {
        match self {
            StateMachineTypedElement::StateEntry(_) => "entry actions",
            StateMachineTypedElement::StateExit(_) => "exit actions",
            StateMachineTypedElement::InitialTransition(_) => "initial transition",
            StateMachineTypedElement::StateTransition(_) => "state transition",
            StateMachineTypedElement::Choice(_) => "choice",
        }
    }
}
