use crate::analyzers::state_machine::transition_expr_analyzer::transition_expr_analyzer_hooks;
use crate::analyzers::state_machine::{
    SmResult, StateMachineAnalysisVisitor, TransitionExprAnalyzer,
};
use crate::passes::state_machine::ConstructFlattenedTransition;
use crate::semantics::state_machine::{
    StateMachineAnalysis, StateMachineSymbol, StateOrChoice, Transition,
};
use fpp_ast::{AstNode, DefStateMachine, TransitionExpr};
use std::ops::ControlFlow;

/// Compute the flattened choice transition map
pub struct ComputeFlattenedChoiceTransitionMap;

impl ComputeFlattenedChoiceTransitionMap {
    /// Analyze a state machine definition
    pub fn def_state_machine(sma: &mut StateMachineAnalysis, node: &DefStateMachine) -> SmResult {
        StateMachineAnalysisVisitor::def_state_machine(
            &ComputeFlattenedChoiceTransitionMap,
            sma,
            node,
        )
    }
}

impl TransitionExprAnalyzer for ComputeFlattenedChoiceTransitionMap {
    fn choice_transition_expr(
        &self,
        sma: &mut StateMachineAnalysis,
        choice: &StateMachineSymbol,
        expr_node: &TransitionExpr,
    ) -> SmResult {
        let transition = {
            let actions = match &expr_node.actions {
                Some(do_expr) => do_expr
                    .actions
                    .iter()
                    .map(|a| sma.get_action_symbol(a))
                    .collect(),
                None => Vec::new(),
            };
            let target = sma.get_state_or_choice(&expr_node.target);
            let transition0 = Transition::External { actions, target };
            let source = StateOrChoice::Choice(choice.clone());
            let cft = ConstructFlattenedTransition::new(sma, source);
            cft.transition(transition0)
        };
        sma.flattened_choice_transition_map
            .insert(expr_node.id(), transition);
        ControlFlow::Continue(())
    }
}

impl StateMachineAnalysisVisitor for ComputeFlattenedChoiceTransitionMap {
    transition_expr_analyzer_hooks!();
}
