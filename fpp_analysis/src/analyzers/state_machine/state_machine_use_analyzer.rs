use crate::analyzers::state_machine::SmResult;
use crate::semantics::state_machine::StateMachineAnalysis;
use fpp_ast::{
    DefChoice, Ident, QualIdent, SpecInitialTransition, SpecStateEntry, SpecStateExit,
    SpecStateTransition, TransitionExpr, TransitionOrDo,
};
use std::ops::ControlFlow;

/// Analyze state machine uses
pub trait StateMachineUseAnalyzer {
    // ----------------------------------------------------------------------
    // Interface methods to override
    // Each of these methods is called when a corresponding use occurs
    // ----------------------------------------------------------------------

    /// A use of an action definition
    fn action_use(&self, sma: &mut StateMachineAnalysis, _node: &Ident) -> SmResult {
        let _ = sma;
        ControlFlow::Continue(())
    }

    /// A use of a guard definition
    fn guard_use(&self, sma: &mut StateMachineAnalysis, _node: &Ident) -> SmResult {
        let _ = sma;
        ControlFlow::Continue(())
    }

    /// A use of a signal definition
    fn signal_use(&self, sma: &mut StateMachineAnalysis, _node: &Ident) -> SmResult {
        let _ = sma;
        ControlFlow::Continue(())
    }

    /// A use of a state definition or choice definition
    fn state_or_choice_use(&self, sma: &mut StateMachineAnalysis, _node: &QualIdent) -> SmResult {
        let _ = sma;
        ControlFlow::Continue(())
    }

    // ----------------------------------------------------------------------
    // Implementation using StateMachineAnalysisVisitor
    // ----------------------------------------------------------------------

    fn def_choice_uses(&self, sma: &mut StateMachineAnalysis, node: &DefChoice) -> SmResult {
        self.guard_use(sma, &node.guard)?;
        self.transition_expr(sma, &node.if_transition)?;
        self.transition_expr(sma, &node.else_transition)
    }

    fn spec_state_entry_uses(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &SpecStateEntry,
    ) -> SmResult {
        self.actions(sma, &node.actions.actions)
    }

    fn spec_state_exit_uses(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &SpecStateExit,
    ) -> SmResult {
        self.actions(sma, &node.actions.actions)
    }

    fn spec_initial_transition_uses(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &SpecInitialTransition,
    ) -> SmResult {
        self.transition_expr(sma, &node.transition)
    }

    fn spec_state_transition_uses(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &SpecStateTransition,
    ) -> SmResult {
        self.signal_use(sma, &node.signal)?;
        if let Some(guard) = &node.guard {
            self.guard_use(sma, guard)?;
        }
        self.transition_or_do(sma, &node.transition_or_do)
    }

    // ------------------------------------------------------------------
    // Private helper methods
    // ------------------------------------------------------------------

    fn transition_expr(&self, sma: &mut StateMachineAnalysis, e: &TransitionExpr) -> SmResult {
        if let Some(do_expr) = &e.actions {
            self.actions(sma, &do_expr.actions)?;
        }
        self.state_or_choice_use(sma, &e.target)
    }

    fn actions(&self, sma: &mut StateMachineAnalysis, actions: &[Ident]) -> SmResult {
        for action in actions {
            self.action_use(sma, action)?;
        }
        ControlFlow::Continue(())
    }

    fn transition_or_do(&self, sma: &mut StateMachineAnalysis, tod: &TransitionOrDo) -> SmResult {
        match tod {
            TransitionOrDo::Transition(e) => self.transition_expr(sma, e),
            TransitionOrDo::Do(actions) => self.actions(sma, &actions.actions),
        }
    }
}
