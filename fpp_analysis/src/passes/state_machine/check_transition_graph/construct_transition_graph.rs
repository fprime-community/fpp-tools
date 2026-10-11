use crate::analyzers::state_machine::transition_expr_analyzer::{
    self, transition_expr_analyzer_hooks,
};
use crate::analyzers::state_machine::{
    SmResult, StateMachineAnalysisVisitor, TransitionExprAnalyzer,
};
use crate::semantics::state_machine::transition_graph::{Arc as TgArc, Node as TgNode};
use crate::semantics::state_machine::{StateMachineAnalysis, StateMachineSymbol, StateOrChoice};
use fpp_ast::{
    AstNode, DefChoice, DefState, SpecInitialTransition, SpecStateTransition, TransitionExpr,
};
use std::ops::ControlFlow;
use std::sync::Arc;

/// Construct the transition graph
pub struct ConstructTransitionGraph;

impl ConstructTransitionGraph {
    /// Analyze a state machine definition
    pub fn def_state_machine(
        sma: &mut StateMachineAnalysis,
        node: &fpp_ast::DefStateMachine,
    ) -> SmResult {
        StateMachineAnalysisVisitor::def_state_machine(&ConstructTransitionGraph, sma, node)
    }
}

impl StateMachineAnalysisVisitor for ConstructTransitionGraph {
    fn def_state(&self, sma: &mut StateMachineAnalysis, node: &DefState) -> SmResult {
        let sym = StateMachineSymbol::State(Arc::new(node.clone()));
        let soc = StateOrChoice::State(sym);
        let graph_node = TgNode { soc };
        sma.transition_graph = sma.transition_graph.add_node(graph_node);
        transition_expr_analyzer::def_state(self, sma, node)
    }

    fn def_choice(&self, sma: &mut StateMachineAnalysis, node: &DefChoice) -> SmResult {
        let sym = StateMachineSymbol::Choice(Arc::new(node.clone()));
        let soc = StateOrChoice::Choice(sym);
        let graph_node = TgNode { soc };
        sma.transition_graph = sma.transition_graph.add_node(graph_node);
        transition_expr_analyzer::def_choice(self, sma, node)
    }

    transition_expr_analyzer_hooks!(spec_initial_transition, spec_state_transition);
}

impl TransitionExprAnalyzer for ConstructTransitionGraph {
    fn state_transition_expr(
        &self,
        sma: &mut StateMachineAnalysis,
        a_node: &SpecStateTransition,
        expr_node: &TransitionExpr,
    ) -> SmResult {
        let end_node = Self::get_node_from_expr(sma, expr_node);
        let arc = TgArc::State {
            start_state: sma.parent_state.clone().unwrap(),
            a_node: Arc::new(a_node.clone()),
            end_node,
        };
        sma.transition_graph = sma.transition_graph.add_arc(arc);
        ControlFlow::Continue(())
    }

    fn initial_transition_expr(
        &self,
        sma: &mut StateMachineAnalysis,
        a_node: &SpecInitialTransition,
        expr_node: &TransitionExpr,
    ) -> SmResult {
        // Construct the end node
        let end_node = Self::get_node_from_expr(sma, expr_node);
        // Update the transition graph
        sma.transition_graph = match &sma.parent_state {
            // We are in a state S. Record the arc from S.
            Some(start_state) => {
                let arc = TgArc::Initial {
                    start_state: start_state.clone(),
                    a_node: Arc::new(a_node.clone()),
                    end_node,
                };
                sma.transition_graph.add_arc(arc)
            }
            // We are not in a state, so this is the state machine initial
            // transition. Record it.
            None => {
                let mut tg = sma.transition_graph.clone();
                tg.initial_node = Some(end_node);
                tg
            }
        };
        ControlFlow::Continue(())
    }

    fn choice_transition_expr(
        &self,
        sma: &mut StateMachineAnalysis,
        choice: &StateMachineSymbol,
        expr_node: &TransitionExpr,
    ) -> SmResult {
        let end_node = Self::get_node_from_expr(sma, expr_node);
        let arc = TgArc::Choice {
            start_choice: choice.clone(),
            a_node: Arc::new(expr_node.clone()),
            end_node,
        };
        sma.transition_graph = sma.transition_graph.add_arc(arc);
        ControlFlow::Continue(())
    }
}

impl ConstructTransitionGraph {
    fn get_node_from_expr(sma: &StateMachineAnalysis, expr_node: &TransitionExpr) -> TgNode {
        let sym = sma.use_def_map.get(&expr_node.target.id()).unwrap();
        let soc = match sym {
            StateMachineSymbol::State(_) => StateOrChoice::State(sym.clone()),
            StateMachineSymbol::Choice(_) => StateOrChoice::Choice(sym.clone()),
            _ => panic!("transition should go to state or choice"),
        };
        TgNode { soc }
    }
}
