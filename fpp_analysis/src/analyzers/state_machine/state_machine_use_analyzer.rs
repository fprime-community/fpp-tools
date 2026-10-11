use crate::analyzers::state_machine::{SmResult, StateMachineAnalysisVisitor};
use crate::semantics::QualifiedName;
use crate::semantics::state_machine::StateMachineAnalysis;
use fpp_ast::{
    DefChoice, Ident, QualIdent, SpecInitialTransition, SpecStateEntry, SpecStateExit,
    SpecStateTransition, TransitionExpr, TransitionOrDo,
};
use std::ops::ControlFlow;

/// Analyze state machine uses
///
/// An analyzer installs the visitor hooks of this module by invoking
/// `state_machine_use_analyzer_hooks!` in its `StateMachineAnalysisVisitor`
/// impl.
pub trait StateMachineUseAnalyzer: StateMachineAnalysisVisitor {
    // ----------------------------------------------------------------------
    // Interface methods to override
    // Each of these methods is called when a corresponding use occurs
    // ----------------------------------------------------------------------

    /// A use of an action definition
    fn action_use(&self, sma: &mut StateMachineAnalysis, node: &Ident, name: &str) -> SmResult {
        let _ = (node, name);
        self.default(sma)
    }

    /// A use of a guard definition
    fn guard_use(&self, sma: &mut StateMachineAnalysis, node: &Ident, name: &str) -> SmResult {
        let _ = (node, name);
        self.default(sma)
    }

    /// A use of a signal definition
    fn signal_use(&self, sma: &mut StateMachineAnalysis, node: &Ident, name: &str) -> SmResult {
        let _ = (node, name);
        self.default(sma)
    }

    /// A use of a state definition or choice definition
    fn state_or_choice_use(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> SmResult {
        let _ = (node, name);
        self.default(sma)
    }
}

// ----------------------------------------------------------------------
// Implementation using StateMachineAnalysisVisitor
// ----------------------------------------------------------------------

/// Implement the `StateMachineAnalysisVisitor` hooks of a
/// [`StateMachineUseAnalyzer`] with the functions of this module
macro_rules! state_machine_use_analyzer_hooks {
    () => {
        fn def_choice(
            &self,
            sma: &mut $crate::semantics::state_machine::StateMachineAnalysis,
            node: &fpp_ast::DefChoice,
        ) -> $crate::analyzers::state_machine::SmResult {
            $crate::analyzers::state_machine::state_machine_use_analyzer::def_choice(
                self, sma, node,
            )
        }

        fn spec_state_entry(
            &self,
            sma: &mut $crate::semantics::state_machine::StateMachineAnalysis,
            node: &fpp_ast::SpecStateEntry,
        ) -> $crate::analyzers::state_machine::SmResult {
            $crate::analyzers::state_machine::state_machine_use_analyzer::spec_state_entry(
                self, sma, node,
            )
        }

        fn spec_state_exit(
            &self,
            sma: &mut $crate::semantics::state_machine::StateMachineAnalysis,
            node: &fpp_ast::SpecStateExit,
        ) -> $crate::analyzers::state_machine::SmResult {
            $crate::analyzers::state_machine::state_machine_use_analyzer::spec_state_exit(
                self, sma, node,
            )
        }

        fn spec_initial_transition(
            &self,
            sma: &mut $crate::semantics::state_machine::StateMachineAnalysis,
            node: &fpp_ast::SpecInitialTransition,
        ) -> $crate::analyzers::state_machine::SmResult {
            $crate::analyzers::state_machine::state_machine_use_analyzer::spec_initial_transition(
                self, sma, node,
            )
        }

        fn spec_state_transition(
            &self,
            sma: &mut $crate::semantics::state_machine::StateMachineAnalysis,
            node: &fpp_ast::SpecStateTransition,
        ) -> $crate::analyzers::state_machine::SmResult {
            $crate::analyzers::state_machine::state_machine_use_analyzer::spec_state_transition(
                self, sma, node,
            )
        }
    };
}
pub(crate) use state_machine_use_analyzer_hooks;

pub fn def_choice<V: StateMachineUseAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &DefChoice,
) -> SmResult {
    ident_node(v, V::guard_use, sma, &node.guard)?;
    transition_expr(v, sma, &node.if_transition)?;
    transition_expr(v, sma, &node.else_transition)
}

pub fn spec_state_entry<V: StateMachineUseAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &SpecStateEntry,
) -> SmResult {
    actions(v, sma, &node.actions.actions)
}

pub fn spec_state_exit<V: StateMachineUseAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &SpecStateExit,
) -> SmResult {
    actions(v, sma, &node.actions.actions)
}

pub fn spec_initial_transition<V: StateMachineUseAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &SpecInitialTransition,
) -> SmResult {
    transition_expr(v, sma, &node.transition)
}

pub fn spec_state_transition<V: StateMachineUseAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &SpecStateTransition,
) -> SmResult {
    ident_node(v, V::signal_use, sma, &node.signal)?;
    if let Some(guard) = &node.guard {
        ident_node(v, V::guard_use, sma, guard)?;
    }
    transition_or_do(v, sma, &node.transition_or_do)
}

// ----------------------------------------------------------------------
// Private helper methods
// ----------------------------------------------------------------------

fn transition_expr<V: StateMachineUseAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    e: &TransitionExpr,
) -> SmResult {
    if let Some(do_expr) = &e.actions {
        actions(v, sma, &do_expr.actions)?;
    }
    qual_ident_node(v, V::state_or_choice_use, sma, &e.target)
}

fn actions<V: StateMachineUseAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    actions: &[Ident],
) -> SmResult {
    for action in actions {
        ident_node(v, V::action_use, sma, action)?;
    }
    ControlFlow::Continue(())
}

fn transition_or_do<V: StateMachineUseAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    tod: &TransitionOrDo,
) -> SmResult {
    match tod {
        TransitionOrDo::Transition(e) => transition_expr(v, sma, e),
        TransitionOrDo::Do(do_expr) => actions(v, sma, &do_expr.actions),
    }
}

fn ident_node<V: StateMachineUseAnalyzer + ?Sized>(
    v: &V,
    f: impl Fn(&V, &mut StateMachineAnalysis, &Ident, &str) -> SmResult,
    sma: &mut StateMachineAnalysis,
    ident: &Ident,
) -> SmResult {
    f(v, sma, ident, &ident.data)
}

fn qual_ident_node<V: StateMachineUseAnalyzer + ?Sized>(
    v: &V,
    f: impl Fn(&V, &mut StateMachineAnalysis, &QualIdent, QualifiedName) -> SmResult,
    sma: &mut StateMachineAnalysis,
    qual_ident: &QualIdent,
) -> SmResult {
    let name = QualifiedName::from(qual_ident);
    f(v, sma, qual_ident, name)
}
