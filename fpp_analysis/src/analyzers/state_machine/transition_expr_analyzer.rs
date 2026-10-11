use crate::analyzers::state_machine::{SmResult, StateMachineAnalysisVisitor, state_analyzer};
use crate::semantics::state_machine::{StateMachineAnalysis, StateMachineSymbol};
use fpp_ast::{
    DefChoice, DefState, SpecInitialTransition, SpecStateTransition, TransitionExpr, TransitionOrDo,
};
use std::ops::ControlFlow;
use std::sync::Arc;

/// Analyze transition expressions
pub trait TransitionExprAnalyzer {
    // ----------------------------------------------------------------------
    // Interface methods to override
    // Each of these methods is called when a corresponding transition
    // expression is visited
    // ----------------------------------------------------------------------

    /// A transition expression in an external state transition
    fn state_transition_expr(
        &self,
        sma: &mut StateMachineAnalysis,
        _a_node: &SpecStateTransition,
        _expr_node: &TransitionExpr,
    ) -> SmResult {
        let _ = sma;
        ControlFlow::Continue(())
    }

    /// A transition expression in an initial transition
    fn initial_transition_expr(
        &self,
        sma: &mut StateMachineAnalysis,
        _a_node: &SpecInitialTransition,
        _expr_node: &TransitionExpr,
    ) -> SmResult {
        let _ = sma;
        ControlFlow::Continue(())
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
        _choice: &StateMachineSymbol,
        _expr_node: &TransitionExpr,
    ) -> SmResult {
        let _ = sma;
        ControlFlow::Continue(())
    }

    // ----------------------------------------------------------------------
    // Implementation using StateMachineAnalysisVisitor
    // ----------------------------------------------------------------------

    fn def_choice_texpr(&self, sma: &mut StateMachineAnalysis, node: &DefChoice) -> SmResult {
        let choice = StateMachineSymbol::Choice(Arc::new(node.clone()));
        self.choice_transition_expr_pair(sma, &choice, &node.if_transition, &node.else_transition)
    }

    /// Analyze a state definition with the state as the parent state
    fn def_state_texpr(&self, sma: &mut StateMachineAnalysis, node: &DefState) -> SmResult
    where
        Self: StateMachineAnalysisVisitor,
    {
        let parent_state = sma.parent_state.clone();
        sma.parent_state = Some(StateMachineSymbol::State(Arc::new(node.clone())));
        state_analyzer::def_state(self, sma, node)?;
        sma.parent_state = parent_state;
        ControlFlow::Continue(())
    }

    fn spec_initial_transition_texpr(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &SpecInitialTransition,
    ) -> SmResult {
        self.initial_transition_expr(sma, node, &node.transition)
    }

    fn spec_state_transition_texpr(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &SpecStateTransition,
    ) -> SmResult {
        match &node.transition_or_do {
            TransitionOrDo::Transition(transition) => {
                self.state_transition_expr(sma, node, transition)
            }
            _ => ControlFlow::Continue(()),
        }
    }
}
