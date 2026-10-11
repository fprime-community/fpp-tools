use crate::analyzers::state_machine::{SmResult, StateMachineAnalysisVisitor, state_analyzer};
use crate::errors::SemanticError;
use crate::semantics::state_machine::{StateMachineAnalysis, StateMachineSymbol};
use fpp_ast::{AstNode, DefState, SpecStateTransition, StateMember};
use fpp_core::Spanned;
use rustc_hash::FxHashMap as HashMap;

/// Check signal uses
pub struct CheckSignalUses;

impl CheckSignalUses {
    /// Analyze a state machine definition
    pub fn def_state_machine(
        sma: &mut StateMachineAnalysis,
        node: &fpp_ast::DefStateMachine,
    ) -> SmResult {
        StateMachineAnalysisVisitor::def_state_machine(&CheckSignalUses, sma, node)
    }
}

impl StateMachineAnalysisVisitor for CheckSignalUses {
    fn def_state(&self, sma: &mut StateMachineAnalysis, node: &DefState) -> SmResult {
        let mut initial_map: HashMap<StateMachineSymbol, &SpecStateTransition> = HashMap::default();
        for member in &node.members {
            if let StateMember::SpecStateTransition(st) = member {
                let sym = sma.use_def_map.get(&st.signal.id()).unwrap().clone();
                match initial_map.get(&sym) {
                    Some(prev_st) => {
                        SemanticError::DuplicateSignal {
                            sig_name: sym.get_unqualified_name().to_string(),
                            state_name: node.name.data.clone(),
                            loc: st.signal.span(),
                            prev_loc: prev_st.signal.span(),
                        }
                        .emit();
                    }
                    None => {
                        initial_map.insert(sym, st);
                    }
                }
            }
        }
        // Visit members
        state_analyzer::def_state(self, sma, node)
    }
}
