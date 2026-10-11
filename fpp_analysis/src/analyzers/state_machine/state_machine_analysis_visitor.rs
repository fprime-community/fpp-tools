use crate::analyzers::state_machine::state_analyzer;
use crate::semantics::state_machine::StateMachineAnalysis;
use fpp_ast::{
    DefAction, DefChoice, DefGuard, DefSignal, DefState, DefStateMachine, SpecInitialTransition,
    SpecStateEntry, SpecStateExit, SpecStateTransition, StateMachineMember, StateMember,
};
use std::ops::ControlFlow;

/// The result type of a state machine analysis step
pub type SmResult = ControlFlow<(), ()>;

/// A generic analysis visitor for state machine semantics
pub trait StateMachineAnalysisVisitor {
    /// The default action: return the analysis unchanged
    fn default(&self, sma: &mut StateMachineAnalysis) -> SmResult {
        let _ = sma;
        ControlFlow::Continue(())
    }

    fn def_action(&self, sma: &mut StateMachineAnalysis, _node: &DefAction) -> SmResult {
        self.default(sma)
    }

    fn def_guard(&self, sma: &mut StateMachineAnalysis, _node: &DefGuard) -> SmResult {
        self.default(sma)
    }

    fn def_choice(&self, sma: &mut StateMachineAnalysis, _node: &DefChoice) -> SmResult {
        self.default(sma)
    }

    fn def_signal(&self, sma: &mut StateMachineAnalysis, _node: &DefSignal) -> SmResult {
        self.default(sma)
    }

    fn spec_initial_transition(
        &self,
        sma: &mut StateMachineAnalysis,
        _node: &SpecInitialTransition,
    ) -> SmResult {
        self.default(sma)
    }

    fn spec_state_entry(&self, sma: &mut StateMachineAnalysis, _node: &SpecStateEntry) -> SmResult {
        self.default(sma)
    }

    fn spec_state_exit(&self, sma: &mut StateMachineAnalysis, _node: &SpecStateExit) -> SmResult {
        self.default(sma)
    }

    fn spec_state_transition(
        &self,
        sma: &mut StateMachineAnalysis,
        _node: &SpecStateTransition,
    ) -> SmResult {
        self.default(sma)
    }

    /// Analyze a state definition. Every state machine analysis analyzes the
    /// state definition members ([`state_analyzer::def_state`]).
    fn def_state(&self, sma: &mut StateMachineAnalysis, node: &DefState) -> SmResult {
        state_analyzer::def_state(self, sma, node)
    }

    /// Analyze a state machine definition: an internal definition goes to
    /// `def_state_machine_internal`, and an external one has no members to visit
    fn def_state_machine(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &DefStateMachine,
    ) -> SmResult {
        match &node.members {
            Some(members) => self.def_state_machine_internal(sma, node, members),
            None => self.default(sma),
        }
    }

    /// Analyze an internal state machine definition, visiting its members
    fn def_state_machine_internal(
        &self,
        sma: &mut StateMachineAnalysis,
        _node: &DefStateMachine,
        members: &[StateMachineMember],
    ) -> SmResult {
        self.visit_sm_members(sma, members)
    }

    fn visit_sm_members(
        &self,
        sma: &mut StateMachineAnalysis,
        members: &[StateMachineMember],
    ) -> SmResult {
        for member in members {
            self.match_state_machine_member(sma, member)?;
        }

        ControlFlow::Continue(())
    }

    fn visit_state_members(
        &self,
        sma: &mut StateMachineAnalysis,
        members: &[StateMember],
    ) -> SmResult {
        for member in members {
            self.match_state_member(sma, member)?;
        }

        ControlFlow::Continue(())
    }

    fn match_state_machine_member(
        &self,
        sma: &mut StateMachineAnalysis,
        member: &StateMachineMember,
    ) -> SmResult {
        match member {
            StateMachineMember::DefAction(n) => self.def_action(sma, n),
            StateMachineMember::DefGuard(n) => self.def_guard(sma, n),
            StateMachineMember::DefChoice(n) => self.def_choice(sma, n),
            StateMachineMember::DefSignal(n) => self.def_signal(sma, n),
            StateMachineMember::DefState(n) => self.def_state(sma, n),
            StateMachineMember::SpecInitialTransition(n) => self.spec_initial_transition(sma, n),
            // Type definitions are handled by the main analysis
            _ => ControlFlow::Continue(()),
        }
    }

    fn match_state_member(&self, sma: &mut StateMachineAnalysis, member: &StateMember) -> SmResult {
        match member {
            StateMember::DefChoice(n) => self.def_choice(sma, n),
            StateMember::DefState(n) => self.def_state(sma, n),
            StateMember::SpecInitialTransition(n) => self.spec_initial_transition(sma, n),
            StateMember::SpecStateEntry(n) => self.spec_state_entry(sma, n),
            StateMember::SpecStateExit(n) => self.spec_state_exit(sma, n),
            StateMember::SpecStateTransition(n) => self.spec_state_transition(sma, n),
            StateMember::SpecInclude(_) => ControlFlow::Continue(()),
        }
    }
}
