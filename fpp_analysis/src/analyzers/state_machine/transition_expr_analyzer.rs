use crate::analyzers::state_machine::{SmResult, StateMachineAnalysisVisitor, state_analyzer};
use crate::semantics::state_machine::{StateMachineAnalysis, StateMachineSymbol};
use fpp_ast::{
    DefChoice, DefState, SpecInitialTransition, SpecStateTransition, TransitionExpr, TransitionOrDo,
};
use std::ops::ControlFlow;
use std::sync::Arc;

/// Analyze transition expressions
///
/// An analyzer installs the visitor hooks of this module by invoking
/// `transition_expr_analyzer_hooks!` in its `StateMachineAnalysisVisitor`
/// impl.
pub trait TransitionExprAnalyzer: StateMachineAnalysisVisitor {
    // ----------------------------------------------------------------------
    // Interface methods to override
    // Each of these methods is called when a corresponding transition
    // expression is visited
    // ----------------------------------------------------------------------

    /// A transition expression in an external state transition
    fn state_transition_expr(
        &self,
        sma: &mut StateMachineAnalysis,
        a_node: &SpecStateTransition,
        expr_node: &TransitionExpr,
    ) -> SmResult {
        let _ = (a_node, expr_node);
        self.default(sma)
    }

    /// A transition expression in an initial transition
    fn initial_transition_expr(
        &self,
        sma: &mut StateMachineAnalysis,
        a_node: &SpecInitialTransition,
        expr_node: &TransitionExpr,
    ) -> SmResult {
        let _ = (a_node, expr_node);
        self.default(sma)
    }

    /// The pair of transition expressions in a choice
    fn choice_transition_expr_pair(
        &self,
        sma: &mut StateMachineAnalysis,
        choice: &StateMachineSymbol,
        if_expr_node: &TransitionExpr,
        else_expr_node: &TransitionExpr,
    ) -> SmResult {
        self.choice_transition_expr(sma, choice, if_expr_node)?;
        self.choice_transition_expr(sma, choice, else_expr_node)
    }

    /// A transition expression in a choice
    fn choice_transition_expr(
        &self,
        sma: &mut StateMachineAnalysis,
        choice: &StateMachineSymbol,
        expr_node: &TransitionExpr,
    ) -> SmResult {
        let _ = (choice, expr_node);
        self.default(sma)
    }
}

// ----------------------------------------------------------------------
// Implementation using StateMachineAnalysisVisitor
// ----------------------------------------------------------------------

/// Implement the `StateMachineAnalysisVisitor` hooks of a
/// [`TransitionExprAnalyzer`] with the functions of this module
///
/// With no arguments it implements every hook. An analyzer that overrides some
/// of the hooks names the others, and its overrides call the functions of this
/// module for the default.
macro_rules! transition_expr_analyzer_hooks {
    () => {
        $crate::analyzers::state_machine::transition_expr_analyzer::transition_expr_analyzer_hooks!(
            def_choice,
            def_state,
            spec_initial_transition,
            spec_state_transition
        );
    };
    ($($hook:ident),+) => {
        $(
            $crate::analyzers::state_machine::transition_expr_analyzer::transition_expr_analyzer_hooks!(
                @hook $hook
            );
        )+
    };
    (@hook def_choice) => {
        fn def_choice(
            &self,
            sma: &mut $crate::semantics::state_machine::StateMachineAnalysis,
            node: &fpp_ast::DefChoice,
        ) -> $crate::analyzers::state_machine::SmResult {
            $crate::analyzers::state_machine::transition_expr_analyzer::def_choice(self, sma, node)
        }
    };
    (@hook def_state) => {
        fn def_state(
            &self,
            sma: &mut $crate::semantics::state_machine::StateMachineAnalysis,
            node: &fpp_ast::DefState,
        ) -> $crate::analyzers::state_machine::SmResult {
            $crate::analyzers::state_machine::transition_expr_analyzer::def_state(self, sma, node)
        }
    };
    (@hook spec_initial_transition) => {
        fn spec_initial_transition(
            &self,
            sma: &mut $crate::semantics::state_machine::StateMachineAnalysis,
            node: &fpp_ast::SpecInitialTransition,
        ) -> $crate::analyzers::state_machine::SmResult {
            $crate::analyzers::state_machine::transition_expr_analyzer::spec_initial_transition(
                self, sma, node,
            )
        }
    };
    (@hook spec_state_transition) => {
        fn spec_state_transition(
            &self,
            sma: &mut $crate::semantics::state_machine::StateMachineAnalysis,
            node: &fpp_ast::SpecStateTransition,
        ) -> $crate::analyzers::state_machine::SmResult {
            $crate::analyzers::state_machine::transition_expr_analyzer::spec_state_transition(
                self, sma, node,
            )
        }
    };
}
pub(crate) use transition_expr_analyzer_hooks;

pub fn def_choice<V: TransitionExprAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &DefChoice,
) -> SmResult {
    let choice = StateMachineSymbol::Choice(Arc::new(node.clone()));
    v.choice_transition_expr_pair(sma, &choice, &node.if_transition, &node.else_transition)
}

/// Analyze a state definition with the state as the parent state
pub fn def_state<V: TransitionExprAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &DefState,
) -> SmResult {
    let parent_state = sma.parent_state.clone();
    sma.parent_state = Some(StateMachineSymbol::State(Arc::new(node.clone())));
    state_analyzer::def_state(v, sma, node)?;
    sma.parent_state = parent_state;
    ControlFlow::Continue(())
}

pub fn spec_initial_transition<V: TransitionExprAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &SpecInitialTransition,
) -> SmResult {
    v.initial_transition_expr(sma, node, &node.transition)
}

pub fn spec_state_transition<V: TransitionExprAnalyzer + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &SpecStateTransition,
) -> SmResult {
    match &node.transition_or_do {
        TransitionOrDo::Transition(transition) => v.state_transition_expr(sma, node, transition),
        _ => ControlFlow::Continue(()),
    }
}
