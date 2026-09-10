mod name_group;
pub use name_group::*;

mod symbol;
pub use symbol::*;

mod scope;
pub use scope::*;

mod state;
pub use state::*;

mod state_or_choice;
pub use state_or_choice::*;

mod transition;
pub use transition::*;

mod typed_element;
pub use typed_element::*;

mod type_option;
pub use type_option::*;

pub mod transition_graph;
pub use transition_graph::TransitionGraph;

mod analysis;
pub use analysis::*;

use crate::semantics::Symbol;
use fpp_ast::{
    DefAction, DefGuard, DefSignal, DefState, DefStateMachine, SpecInitialTransition,
    StateMachineMember, StateMember,
};
use std::sync::Arc;

/// The kind of a state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    External,
    Internal,
}

/// An FPP state machine
#[derive(Debug, Clone)]
pub struct StateMachine {
    /// The AST node defining the state machine
    pub node: Arc<DefStateMachine>,
    /// The state machine analysis
    pub sma: StateMachineAnalysis,
    pub actions: Vec<StateMachineSymbol>,
    pub guards: Vec<StateMachineSymbol>,
    pub signals: Vec<StateMachineSymbol>,
}

impl StateMachine {
    pub fn new(node: Arc<DefStateMachine>, sma: StateMachineAnalysis) -> StateMachine {
        let actions = StateMachine::get_actions(&node)
            .into_iter()
            .map(|n| StateMachineSymbol::Action(Arc::new(n.clone())))
            .collect();
        let guards = StateMachine::get_guards(&node)
            .into_iter()
            .map(|n| StateMachineSymbol::Guard(Arc::new(n.clone())))
            .collect();
        let signals = StateMachine::get_signals(&node)
            .into_iter()
            .map(|n| StateMachineSymbol::Signal(Arc::new(n.clone())))
            .collect();
        StateMachine {
            node,
            sma,
            actions,
            guards,
            signals,
        }
    }

    pub fn has_actions(&self) -> bool {
        !self.actions.is_empty()
    }

    pub fn has_guards(&self) -> bool {
        !self.guards.is_empty()
    }

    pub fn has_signals(&self) -> bool {
        !self.signals.is_empty()
    }

    pub fn get_symbol(&self) -> Symbol {
        Symbol::StateMachine(self.node.clone())
    }

    pub fn get_kind(&self) -> Kind {
        StateMachine::get_symbol_kind(&self.node)
    }

    /// Whether a blocking analysis error was recorded for this state machine.
    pub fn blocking_error(&self) -> bool {
        self.sma.blocking_error
    }

    /// The unqualified names of the leaf states, one per distinct leaf state.
    pub fn leaf_state_names(&self) -> Vec<String> {
        Self::get_leaf_states(&self.node)
            .iter()
            .map(|s| s.name.data.clone())
            .collect()
    }

    pub fn get_symbol_kind(sm: &DefStateMachine) -> Kind {
        match sm.members {
            None => Kind::External,
            Some(_) => Kind::Internal,
        }
    }

    pub fn get_initial_specifier(sm: &DefStateMachine) -> &SpecInitialTransition {
        let specifiers: Vec<&SpecInitialTransition> = sm
            .members
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(|m| match m {
                StateMachineMember::SpecInitialTransition(node) => Some(node),
                _ => None,
            })
            .collect();
        match specifiers.as_slice() {
            [head] => head,
            _ => panic!("state machine must have exactly one initial transition specifier"),
        }
    }

    pub fn get_actions(sm: &DefStateMachine) -> Vec<&DefAction> {
        sm.members
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(|m| match m {
                StateMachineMember::DefAction(node) => Some(node),
                _ => None,
            })
            .collect()
    }

    pub fn get_guards(sm: &DefStateMachine) -> Vec<&DefGuard> {
        sm.members
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(|m| match m {
                StateMachineMember::DefGuard(node) => Some(node),
                _ => None,
            })
            .collect()
    }

    pub fn get_signals(sm: &DefStateMachine) -> Vec<&DefSignal> {
        sm.members
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(|m| match m {
                StateMachineMember::DefSignal(node) => Some(node),
                _ => None,
            })
            .collect()
    }

    /// Gets the leaf states of a state machine.
    pub fn get_leaf_states(sm: &DefStateMachine) -> Vec<Arc<DefState>> {
        let mut states = Vec::new();
        for member in sm.members.as_deref().unwrap_or(&[]) {
            if let StateMachineMember::DefState(node) = member {
                Self::collect_leaf_states(node, &mut states);
            }
        }
        states
    }

    fn collect_leaf_states(state: &DefState, states: &mut Vec<Arc<DefState>>) {
        let substates: Vec<&DefState> = state
            .members
            .iter()
            .filter_map(|m| match m {
                StateMember::DefState(node) => Some(node),
                _ => None,
            })
            .collect();
        if substates.is_empty() {
            states.push(Arc::new(state.clone()));
        } else {
            for substate in substates {
                Self::collect_leaf_states(substate, states);
            }
        }
    }
}
