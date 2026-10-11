use crate::analyzers::state_machine::{SmResult, StateMachineAnalysisVisitor};
use crate::semantics::state_machine::{StateMachineAnalysis, StateMachineTypedElement};
use fpp_ast::{
    DefChoice, SpecInitialTransition, SpecStateEntry, SpecStateExit, SpecStateTransition,
};
use std::sync::Arc;

/// State machine typed element analyzer
///
/// An analyzer installs the visitor hooks of this module by invoking
/// `sm_typed_element_analyzer_hooks!` in its `StateMachineAnalysisVisitor`
/// impl.
pub trait SmTypedElementAnalyzer: StateMachineAnalysisVisitor {
    // ----------------------------------------------------------------------
    // Interface methods to override
    // Each of these methods is called when a corresponding typed element
    // is visited
    // ----------------------------------------------------------------------

    fn initial_transition_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        te: &StateMachineTypedElement,
    ) -> SmResult {
        let _ = te;
        self.default(sma)
    }

    fn choice_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        te: &StateMachineTypedElement,
    ) -> SmResult {
        let _ = te;
        self.default(sma)
    }

    fn state_entry_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        te: &StateMachineTypedElement,
    ) -> SmResult {
        let _ = te;
        self.default(sma)
    }

    fn state_exit_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        te: &StateMachineTypedElement,
    ) -> SmResult {
        let _ = te;
        self.default(sma)
    }

    fn state_transition_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        te: &StateMachineTypedElement,
    ) -> SmResult {
        let _ = te;
        self.default(sma)
    }

    // ----------------------------------------------------------------------
    // Implementation using StateMachineAnalysisVisitor
    // ----------------------------------------------------------------------

    /// Visit a typed element. An override calls the function
    /// `sm_typed_element_analyzer::visit_typed_element` to reach the default.
    fn visit_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        te: StateMachineTypedElement,
    ) -> SmResult {
        visit_typed_element(self, sma, te)
    }
}

/// Implement the `StateMachineAnalysisVisitor` hooks of a
/// [`SmTypedElementAnalyzer`] with the functions of this module
macro_rules! sm_typed_element_analyzer_hooks {
    () => {
        fn def_choice(
            &self,
            sma: &mut $crate::semantics::state_machine::StateMachineAnalysis,
            node: &fpp_ast::DefChoice,
        ) -> $crate::analyzers::state_machine::SmResult {
            $crate::analyzers::state_machine::sm_typed_element_analyzer::def_choice(self, sma, node)
        }

        fn spec_state_entry(
            &self,
            sma: &mut $crate::semantics::state_machine::StateMachineAnalysis,
            node: &fpp_ast::SpecStateEntry,
        ) -> $crate::analyzers::state_machine::SmResult {
            $crate::analyzers::state_machine::sm_typed_element_analyzer::spec_state_entry(
                self, sma, node,
            )
        }

        fn spec_state_exit(
            &self,
            sma: &mut $crate::semantics::state_machine::StateMachineAnalysis,
            node: &fpp_ast::SpecStateExit,
        ) -> $crate::analyzers::state_machine::SmResult {
            $crate::analyzers::state_machine::sm_typed_element_analyzer::spec_state_exit(
                self, sma, node,
            )
        }

        fn spec_initial_transition(
            &self,
            sma: &mut $crate::semantics::state_machine::StateMachineAnalysis,
            node: &fpp_ast::SpecInitialTransition,
        ) -> $crate::analyzers::state_machine::SmResult {
            $crate::analyzers::state_machine::sm_typed_element_analyzer::spec_initial_transition(
                self, sma, node,
            )
        }

        fn spec_state_transition(
            &self,
            sma: &mut $crate::semantics::state_machine::StateMachineAnalysis,
            node: &fpp_ast::SpecStateTransition,
        ) -> $crate::analyzers::state_machine::SmResult {
            $crate::analyzers::state_machine::sm_typed_element_analyzer::spec_state_transition(
                self, sma, node,
            )
        }
    };
}
pub(crate) use sm_typed_element_analyzer_hooks;

/// Visit a typed element: call the hook for its kind
pub fn visit_typed_element<V: SmTypedElementAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    te: StateMachineTypedElement,
) -> SmResult {
    match &te {
        StateMachineTypedElement::InitialTransition(_) => {
            v.initial_transition_typed_element(sma, &te)
        }
        StateMachineTypedElement::Choice(_) => v.choice_typed_element(sma, &te),
        StateMachineTypedElement::StateEntry(_) => v.state_entry_typed_element(sma, &te),
        StateMachineTypedElement::StateExit(_) => v.state_exit_typed_element(sma, &te),
        StateMachineTypedElement::StateTransition(_) => v.state_transition_typed_element(sma, &te),
    }
}

pub fn def_choice<V: SmTypedElementAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &DefChoice,
) -> SmResult {
    v.visit_typed_element(
        sma,
        StateMachineTypedElement::Choice(Arc::new(node.clone())),
    )
}

pub fn spec_state_entry<V: SmTypedElementAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &SpecStateEntry,
) -> SmResult {
    v.visit_typed_element(
        sma,
        StateMachineTypedElement::StateEntry(Arc::new(node.clone())),
    )
}

pub fn spec_state_exit<V: SmTypedElementAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &SpecStateExit,
) -> SmResult {
    v.visit_typed_element(
        sma,
        StateMachineTypedElement::StateExit(Arc::new(node.clone())),
    )
}

pub fn spec_initial_transition<V: SmTypedElementAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &SpecInitialTransition,
) -> SmResult {
    v.visit_typed_element(
        sma,
        StateMachineTypedElement::InitialTransition(Arc::new(node.clone())),
    )
}

pub fn spec_state_transition<V: SmTypedElementAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &SpecStateTransition,
) -> SmResult {
    v.visit_typed_element(
        sma,
        StateMachineTypedElement::StateTransition(Arc::new(node.clone())),
    )
}
