mod construct_transition_graph;
pub use construct_transition_graph::*;

mod check_choice_cycles;
pub use check_choice_cycles::*;

mod check_tg_reachability;
pub use check_tg_reachability::*;
use fpp_ast::DefStateMachine;
use std::ops::ControlFlow;

use crate::{analyzers::state_machine::SmResult, semantics::state_machine::StateMachineAnalysis};

/// Compute and check the transition graph
pub struct CheckTransitionGraph;

impl CheckTransitionGraph {
    /// Analyze a state machine definition. A choice cycle sets
    /// `sma.blocking_error`, and the caller stops before the typed-element pass,
    /// which walks the choice graph assuming it is acyclic.
    pub fn def_state_machine(sma: &mut StateMachineAnalysis, node: &DefStateMachine) -> SmResult {
        ConstructTransitionGraph::def_state_machine(sma, node)?;
        CheckTGReachability::state_machine_analysis(sma)?;
        CheckChoiceCycles::state_machine_analysis(sma)?;
        sma.reverse_transition_graph = sma.transition_graph.get_reverse_graph();
        ControlFlow::Continue(())
    }
}
