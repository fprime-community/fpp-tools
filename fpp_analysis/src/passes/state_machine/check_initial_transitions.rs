use crate::analyzers::state_machine::{SmResult, StateMachineAnalysisVisitor, state_analyzer};
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::SymbolInterface;
use crate::semantics::state_machine::{StateMachineAnalysis, StateMachineSymbol};
use fpp_ast::{
    AstNode, DefState, DefStateMachine, SpecInitialTransition, StateMachineMember, StateMember,
};
use fpp_core::{Span, Spanned};
use rustc_hash::FxHashSet as HashSet;
use std::ops::ControlFlow;

/// Check initial transitions
pub struct CheckInitialTransitions;

impl CheckInitialTransitions {
    /// Analyze a state machine definition
    pub fn def_state_machine(sma: &mut StateMachineAnalysis, node: &DefStateMachine) -> SmResult {
        StateMachineAnalysisVisitor::def_state_machine(&CheckInitialTransitions, sma, node)
    }
}

impl StateMachineAnalysisVisitor for CheckInitialTransitions {
    fn def_state_machine_internal(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &DefStateMachine,
        members: &[StateMachineMember],
    ) -> SmResult {
        // Check that there is exactly one initial transition specifier. The
        // error is blocking: without an initial transition the reachability
        // check has no node to start from.
        let initial_transitions = members
            .iter()
            .filter_map(|m| match m {
                StateMachineMember::SpecInitialTransition(spec_initial_transition) => {
                    Some(spec_initial_transition.span())
                }
                _ => None,
            })
            .collect();
        if let Err(err) = Self::check_one_initial_transition(
            initial_transitions,
            node.name.span(),
            "state machine",
        ) {
            err.emit();
            sma.blocking_error = true;
        }
        // Visit the members
        sma.parent_state = None;
        self.visit_sm_members(sma, members)
    }

    fn spec_initial_transition(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &SpecInitialTransition,
    ) -> SmResult {
        // Check that node leads to a state machine or choice
        // with the same parent symbol as sma
        let dest_id = node.transition.target.id();
        let dest_symbol = sma.use_def_map[&dest_id].clone();
        if let Err(symbols) = Self::check_for_dest_outside_parent(
            sma,
            dest_symbol,
            &sma.parent_state,
            Vec::new(),
            HashSet::default(),
        ) {
            let loc = node.span();
            let msg_head = match sma.parent_state {
                Some(_) => {
                    "initial transition of state must go to state or choice defined in the same state"
                }
                None => {
                    "initial transition of state machine may not go to a state or choice defined in a substate"
                }
            };
            let path: Vec<Span> = symbols.iter().rev().map(|s| s.get_loc()).collect();
            SemanticError::InvalidInitialTransition {
                loc,
                msg: msg_head.to_string(),
                path,
                duplicate: Vec::new(),
            }
            .emit();
        }
        ControlFlow::Continue(())
    }

    fn def_state(&self, sma: &mut StateMachineAnalysis, node: &DefState) -> SmResult {
        let loc = node.name.span();
        let sub_states = node
            .members
            .iter()
            .filter(|m| matches!(m, StateMember::DefState(_)))
            .count();

        let initial_transitions: Vec<Span> = node
            .members
            .iter()
            .filter_map(|m| match m {
                StateMember::SpecInitialTransition(spec_initial_transition) => {
                    Some(spec_initial_transition.span())
                }
                _ => None,
            })
            .collect();

        match (sub_states, initial_transitions.len()) {
            // No substates, no initial transition: OK
            (0, 0) => ControlFlow::Continue(()),
            // Substates or initial transitions: Check semantics
            _ => {
                // Check for exactly one initial transition. The error is
                // blocking: the reachability check would report the substates
                // that the state cannot enter as unreachable.
                if let Err(err) = Self::check_one_initial_transition(
                    initial_transitions,
                    loc,
                    "state with substates",
                ) {
                    err.emit();
                    sma.blocking_error = true;
                }
                // Visit the members
                let parent_state = sma.parent_state.clone();
                sma.parent_state =
                    Some(StateMachineSymbol::State(std::sync::Arc::new(node.clone())));
                state_analyzer::def_state(self, sma, node)?;
                sma.parent_state = parent_state;
                ControlFlow::Continue(())
            }
        }
    }
}

impl CheckInitialTransitions {
    // Checks that there is exactly one initial transition specifier
    fn check_one_initial_transition(
        initial_transitions: Vec<Span>,
        loc: Span,
        def_kind: &str,
    ) -> SemanticResult<()> {
        if initial_transitions.is_empty() {
            Err(SemanticError::InvalidInitialTransition {
                loc,
                msg: format!("{} must have initial transition", def_kind),
                path: Vec::new(),
                duplicate: Vec::new(),
            })
        } else if initial_transitions.len() == 1 {
            Ok(())
        } else {
            Err(SemanticError::InvalidInitialTransition {
                loc,
                msg: format!(
                    "{} has {} initial transitions; only one is allowed",
                    def_kind,
                    initial_transitions.len()
                ),
                path: Vec::new(),
                duplicate: initial_transitions.iter().map(|i| i.span()).collect(),
            })
        }
    }

    // Checks for a transition target outside the parent
    // Returns the symbol path (error) or the set of visited symbols
    fn check_for_dest_outside_parent(
        sma: &StateMachineAnalysis,
        dest_symbol: StateMachineSymbol,
        parent_state: &Option<StateMachineSymbol>,
        error_symbols: Vec<StateMachineSymbol>,
        visited_symbols: HashSet<StateMachineSymbol>,
    ) -> Result<HashSet<StateMachineSymbol>, Vec<StateMachineSymbol>> {
        if visited_symbols.contains(&dest_symbol) {
            // We have visited this symbol already: nothing to do
            return Ok(visited_symbols);
        }
        // We haven't visited the symbol
        let dest_parent_symbol = sma.parent_state_map.get(&dest_symbol).cloned();
        // Check that the symbol is defined in the parent
        let mut visited_symbols = if &dest_parent_symbol == parent_state {
            let mut vs = visited_symbols;
            vs.insert(dest_symbol.clone());
            vs
        } else {
            let mut es = error_symbols.clone();
            es.insert(0, dest_symbol.clone());
            return Err(es);
        };
        // Recursively check choice targets
        if let StateMachineSymbol::Choice(a_node) = &dest_symbol {
            // Choice: check it
            let mut next_error_symbols = error_symbols.clone();
            next_error_symbols.insert(0, dest_symbol.clone());
            // Recursively check the if transition
            let if_dest = &a_node.if_transition.target;
            let if_dest_symbol = sma.use_def_map[&if_dest.id()].clone();
            visited_symbols = Self::check_for_dest_outside_parent(
                sma,
                if_dest_symbol,
                parent_state,
                next_error_symbols.clone(),
                visited_symbols,
            )?;
            // Recursively check the else transition
            let else_dest = &a_node.else_transition.target;
            let else_dest_symbol = sma.use_def_map[&else_dest.id()].clone();
            visited_symbols = Self::check_for_dest_outside_parent(
                sma,
                else_dest_symbol,
                parent_state,
                next_error_symbols,
                visited_symbols,
            )?;
        }
        // Not a choice: nothing to do
        Ok(visited_symbols)
    }
}
