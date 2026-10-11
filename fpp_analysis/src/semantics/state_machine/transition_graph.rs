use crate::semantics::state_machine::{
    StateMachineSymbol, StateMachineTypedElement, StateOrChoice,
};
use fpp_core::{Span, Spanned};
use indexmap::{IndexMap, IndexSet};

/// A node in a transition graph
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Node {
    pub soc: StateOrChoice,
}

/// An arc in a transition graph
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Arc {
    Initial {
        start_state: StateMachineSymbol,
        a_node: std::sync::Arc<fpp_ast::SpecInitialTransition>,
        end_node: Node,
    },
    State {
        start_state: StateMachineSymbol,
        a_node: std::sync::Arc<fpp_ast::SpecStateTransition>,
        end_node: Node,
    },
    Choice {
        start_choice: StateMachineSymbol,
        a_node: std::sync::Arc<fpp_ast::TransitionExpr>,
        end_node: Node,
    },
}

impl Arc {
    pub fn get_start_node(&self) -> Node {
        match self {
            Arc::Initial { start_state, .. } => Node {
                soc: StateOrChoice::State(start_state.clone()),
            },
            Arc::State { start_state, .. } => Node {
                soc: StateOrChoice::State(start_state.clone()),
            },
            Arc::Choice { start_choice, .. } => Node {
                soc: StateOrChoice::Choice(start_choice.clone()),
            },
        }
    }

    pub fn get_end_node(&self) -> &Node {
        match self {
            Arc::Initial { end_node, .. } => end_node,
            Arc::State { end_node, .. } => end_node,
            Arc::Choice { end_node, .. } => end_node,
        }
    }

    pub fn get_typed_element(&self) -> StateMachineTypedElement {
        match self {
            Arc::Initial { a_node, .. } => {
                StateMachineTypedElement::InitialTransition(a_node.clone())
            }
            Arc::State { a_node, .. } => StateMachineTypedElement::StateTransition(a_node.clone()),
            Arc::Choice { start_choice, .. } => match start_choice {
                StateMachineSymbol::Choice(node) => StateMachineTypedElement::Choice(node.clone()),
                _ => panic!("expected choice symbol"),
            },
        }
    }

    pub fn show_kind(&self) -> &'static str {
        match self {
            Arc::Initial { .. } => "initial transition",
            Arc::State { .. } => "state transition",
            Arc::Choice { .. } => "choice transition",
        }
    }

    /// Gets the location of the transition that the arc represents
    pub(crate) fn get_loc(&self) -> Span {
        match self {
            Arc::Initial { a_node, .. } => a_node.span(),
            Arc::State { a_node, .. } => a_node.span(),
            Arc::Choice { a_node, .. } => a_node.span(),
        }
    }

    pub fn show_transition(&self) -> String {
        let span = self.get_loc();
        let end_name = self.get_end_node().soc.get_name();
        let start = span.start();
        format!(
            "{} at {}:{}.{} to {}",
            self.show_kind(),
            span.file(),
            start.line() + 1,
            start.column() + 1,
            end_name
        )
    }
}

/// A map from nodes to their sets of arcs
///
/// The nodes and the arcs of each node iterate in the order they were added,
/// which orders the diagnostics of the passes that walk the graph.
pub type ArcMap = IndexMap<Node, IndexSet<Arc>>;

/// An FPP transition graph
#[derive(Debug, Clone, Default)]
pub struct TransitionGraph {
    pub initial_node: Option<Node>,
    pub arc_map: ArcMap,
}

impl TransitionGraph {
    /// Adds a node to the graph
    pub fn add_node(&self, node: Node) -> TransitionGraph {
        let mut result = self.clone();
        result.arc_map.entry(node).or_default();
        result
    }

    /// Adds an arc to the graph
    pub fn add_arc(&self, arc: Arc) -> TransitionGraph {
        let mut result = self.clone();
        let start_node = arc.get_start_node();
        result.arc_map.entry(start_node).or_default().insert(arc);
        result
    }

    /// Adds a reverse arc to the graph
    pub fn add_reverse_arc(&self, arc: Arc) -> TransitionGraph {
        let mut result = self.clone();
        let end_node = arc.get_end_node().clone();
        result.arc_map.entry(end_node).or_default().insert(arc);
        result
    }

    /// Gets the reverse of this transition graph
    pub fn get_reverse_graph(&self) -> TransitionGraph {
        let mut tg = TransitionGraph {
            initial_node: self.initial_node.clone(),
            arc_map: ArcMap::default(),
        };
        if let Some(node) = &self.initial_node {
            tg.arc_map.insert(node.clone(), IndexSet::new());
        }
        for arcs in self.arc_map.values() {
            for a in arcs {
                tg = tg.add_reverse_arc(a.clone());
            }
        }
        tg
    }
}
