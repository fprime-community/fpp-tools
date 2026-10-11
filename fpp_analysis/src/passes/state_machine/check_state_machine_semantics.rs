use crate::Analysis;
use crate::analyzers::state_machine::SmResult;
use crate::passes::state_machine::{
    CheckInitialTransitions, CheckSignalUses, CheckStateMachineUses, CheckTransitionGraph,
    CheckTypedElements, ComputeFlattenedChoiceTransitionMap, ComputeFlattenedStateTransitionMap,
    EnterStateMachineSymbols,
};
use crate::semantics::state_machine::StateMachineAnalysis;
use fpp_ast::DefStateMachine;
use std::ops::ControlFlow;

/// Check semantics for a state machine definition
pub struct CheckStateMachineSemantics;

impl CheckStateMachineSemantics {
    pub fn def_state_machine(
        a: &Analysis,
        sma: &mut StateMachineAnalysis,
        node: &DefStateMachine,
    ) -> SmResult {
        match node.members {
            // Internal state machine: check it
            Some(_) => {
                // Run full passes and gate future passes when errors arise
                EnterStateMachineSymbols::def_state_machine(sma, node)?;
                CheckStateMachineUses::def_state_machine(sma, node)?;
                // An unresolved use leaves no `use_def_map` entry; every pass
                // below reads that map, so stop here if resolution failed.
                if sma.blocking_error {
                    return ControlFlow::Continue(());
                }
                CheckInitialTransitions::def_state_machine(sma, node)?;
                CheckSignalUses::def_state_machine(sma, node)?;
                // An initial transition error stops the graph passes: without a
                // state machine initial transition there is no `initial_node`
                // for the reachability check to start from.
                if sma.blocking_error {
                    return ControlFlow::Continue(());
                }
                CheckTransitionGraph::def_state_machine(sma, node)?;
                // A choice cycle makes the choice graph non-acyclic; the
                // typed-element pass walks it recursively, so stop here.
                if sma.blocking_error {
                    return ControlFlow::Continue(());
                }
                CheckTypedElements::def_state_machine(a, sma, node)?;
                ComputeFlattenedStateTransitionMap::def_state_machine(sma, node)?;
                ComputeFlattenedChoiceTransitionMap::def_state_machine(sma, node)?;
                ControlFlow::Continue(())
            }
            // External state machine: do nothing
            None => ControlFlow::Continue(()),
        }
    }
}
