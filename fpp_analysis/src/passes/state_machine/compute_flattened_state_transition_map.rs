use crate::analyzers::state_machine::{SmResult, StateMachineAnalysisVisitor, state_analyzer};
use crate::passes::state_machine::ConstructFlattenedTransition;
use crate::semantics::state_machine::{
    GuardedTransition, StateMachineAnalysis, StateMachineSymbol, StateOrChoice, Transition,
};
use fpp_ast::{
    DefState, DefStateMachine, SpecStateTransition, StateMachineMember, StateMember, TransitionOrDo,
};
use std::ops::ControlFlow;
use std::sync::Arc;

/// Compute the flattened state transition map
pub struct ComputeFlattenedStateTransitionMap;

impl ComputeFlattenedStateTransitionMap {
    /// Analyze a state machine definition
    pub fn def_state_machine(sma: &mut StateMachineAnalysis, node: &DefStateMachine) -> SmResult {
        StateMachineAnalysisVisitor::def_state_machine(
            &ComputeFlattenedStateTransitionMap,
            sma,
            node,
        )
    }
}

impl StateMachineAnalysisVisitor for ComputeFlattenedStateTransitionMap {
    fn def_state_machine_internal(
        &self,
        sma: &mut StateMachineAnalysis,
        _node: &DefStateMachine,
        members: &[StateMachineMember],
    ) -> SmResult {
        sma.signal_transition_map = Default::default();
        sma.flattened_state_transition_map = Default::default();
        self.visit_sm_members(sma, members)
    }

    fn def_state(&self, sma: &mut StateMachineAnalysis, node: &DefState) -> SmResult {
        let state_transitions = Self::get_state_transition_specifiers(node);
        let saved_stm = sma.signal_transition_map.clone();
        let mut stm = sma.signal_transition_map.clone();
        for sts in state_transitions {
            let signal = sma.get_signal_symbol(&sts.signal);
            let guard_opt = sts.guard.as_ref().map(|g| sma.get_guard_symbol(g));
            let transition = match &sts.transition_or_do {
                TransitionOrDo::Transition(transition) => {
                    let actions = match &transition.actions {
                        Some(do_expr) => do_expr
                            .actions
                            .iter()
                            .map(|a| sma.get_action_symbol(a))
                            .collect(),
                        None => Vec::new(),
                    };
                    let target = sma.get_state_or_choice(&transition.target);
                    Transition::External { actions, target }
                }
                TransitionOrDo::Do(do_expr) => {
                    let actions = do_expr
                        .actions
                        .iter()
                        .map(|a| sma.get_action_symbol(a))
                        .collect();
                    Transition::Internal { actions }
                }
            };
            let guarded_transition = GuardedTransition {
                guard_opt,
                transition,
            };
            stm.insert(signal, guarded_transition);
        }
        match Self::get_state_defs(node).as_slice() {
            [] => {
                let state = StateMachineSymbol::State(Arc::new(node.clone()));
                let mut fstm = sma.flattened_state_transition_map.clone();
                for (s, gt) in &stm {
                    let soc = StateOrChoice::State(state.clone());
                    let cft = ConstructFlattenedTransition::new(sma, soc);
                    let transition = cft.transition(gt.transition.clone());
                    let gt1 = GuardedTransition {
                        guard_opt: gt.guard_opt.clone(),
                        transition,
                    };
                    let map = fstm.entry(s.clone()).or_default();
                    map.insert(state.clone(), gt1);
                }
                sma.flattened_state_transition_map = fstm;
                ControlFlow::Continue(())
            }
            _ => {
                sma.signal_transition_map = stm;
                state_analyzer::def_state(self, sma, node)?;
                sma.signal_transition_map = saved_stm;
                ControlFlow::Continue(())
            }
        }
    }
}

impl ComputeFlattenedStateTransitionMap {
    fn get_state_transition_specifiers(def_state: &DefState) -> Vec<&SpecStateTransition> {
        def_state
            .members
            .iter()
            .filter_map(|m| match m {
                StateMember::SpecStateTransition(node) => Some(node),
                _ => None,
            })
            .collect()
    }

    fn get_state_defs(def_state: &DefState) -> Vec<&DefState> {
        def_state
            .members
            .iter()
            .filter_map(|m| match m {
                StateMember::DefState(node) => Some(node),
                _ => None,
            })
            .collect()
    }
}
