use crate::analyzers::state_machine::SmResult;
use crate::semantics::state_machine::{StateMachineAnalysis, StateMachineTypedElement};
use fpp_ast::{
    DefChoice, SpecInitialTransition, SpecStateEntry, SpecStateExit, SpecStateTransition,
};
use std::ops::ControlFlow;
use std::sync::Arc;

/// State machine typed element analyzer
pub trait SmTypedElementAnalyzer {
    // ----------------------------------------------------------------------
    // Interface methods to override
    // Each of these methods is called when a corresponding typed element
    // is visited
    // ----------------------------------------------------------------------

    fn initial_transition_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        _te: &StateMachineTypedElement,
    ) -> SmResult {
        let _ = sma;
        ControlFlow::Continue(())
    }

    fn choice_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        _te: &StateMachineTypedElement,
    ) -> SmResult {
        let _ = sma;
        ControlFlow::Continue(())
    }

    fn state_entry_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        _te: &StateMachineTypedElement,
    ) -> SmResult {
        let _ = sma;
        ControlFlow::Continue(())
    }

    fn state_exit_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        _te: &StateMachineTypedElement,
    ) -> SmResult {
        let _ = sma;
        ControlFlow::Continue(())
    }

    fn state_transition_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        _te: &StateMachineTypedElement,
    ) -> SmResult {
        let _ = sma;
        ControlFlow::Continue(())
    }

    // ----------------------------------------------------------------------
    // Implementation using StateMachineAnalysisVisitor
    // ----------------------------------------------------------------------

    fn visit_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        te: StateMachineTypedElement,
    ) -> SmResult {
        self.dispatch_typed_element(sma, te)
    }

    /// The default `visit_typed_element`: call the hook for the kind of the
    /// typed element. An override of `visit_typed_element` calls it to reach
    /// the default.
    fn dispatch_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        te: StateMachineTypedElement,
    ) -> SmResult {
        match &te {
            StateMachineTypedElement::InitialTransition(_) => {
                self.initial_transition_typed_element(sma, &te)
            }
            StateMachineTypedElement::Choice(_) => self.choice_typed_element(sma, &te),
            StateMachineTypedElement::StateEntry(_) => self.state_entry_typed_element(sma, &te),
            StateMachineTypedElement::StateExit(_) => self.state_exit_typed_element(sma, &te),
            StateMachineTypedElement::StateTransition(_) => {
                self.state_transition_typed_element(sma, &te)
            }
        }
    }

    fn def_choice_te(&self, sma: &mut StateMachineAnalysis, node: &DefChoice) -> SmResult {
        self.visit_typed_element(
            sma,
            StateMachineTypedElement::Choice(Arc::new(node.clone())),
        )
    }

    fn spec_state_entry_te(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &SpecStateEntry,
    ) -> SmResult {
        self.visit_typed_element(
            sma,
            StateMachineTypedElement::StateEntry(Arc::new(node.clone())),
        )
    }

    fn spec_state_exit_te(&self, sma: &mut StateMachineAnalysis, node: &SpecStateExit) -> SmResult {
        self.visit_typed_element(
            sma,
            StateMachineTypedElement::StateExit(Arc::new(node.clone())),
        )
    }

    fn spec_initial_transition_te(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &SpecInitialTransition,
    ) -> SmResult {
        self.visit_typed_element(
            sma,
            StateMachineTypedElement::InitialTransition(Arc::new(node.clone())),
        )
    }

    fn spec_state_transition_te(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &SpecStateTransition,
    ) -> SmResult {
        self.visit_typed_element(
            sma,
            StateMachineTypedElement::StateTransition(Arc::new(node.clone())),
        )
    }
}
