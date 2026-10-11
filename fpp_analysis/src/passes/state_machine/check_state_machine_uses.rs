use crate::analyzers::state_machine::state_machine_use_analyzer::state_machine_use_analyzer_hooks;
use crate::analyzers::state_machine::{
    SmResult, StateMachineAnalysisVisitor, StateMachineUseAnalyzer, state_analyzer,
};
use crate::errors::SemanticResult;
use crate::passes::CheckUsesHelpers;
use crate::semantics::QualifiedName;
use crate::semantics::state_machine::{
    SmNameSymbolMap, StateMachineAnalysis, StateMachineNameGroup, StateMachineNameGroupMap,
    StateMachineSymbol,
};
use fpp_ast::{DefState, Ident, QualIdent};
use std::ops::ControlFlow;
use std::sync::Arc;

/// Match uses to their definitions
pub struct CheckStateMachineUses;

/// The check uses helpers for the state machine analysis
const HELPERS: CheckUsesHelpers<
    StateMachineAnalysis,
    StateMachineNameGroup,
    StateMachineSymbol,
    StateMachineNameGroupMap<SmNameSymbolMap>,
> = CheckUsesHelpers {
    get_nested_scope_symbol: |sma, is_absolute, ng, name| {
        sma.nested_scope.get(is_absolute, ng, name)
    },
    get_symbol_scope: |sma, symbol| sma.symbol_scope_map.get(symbol),
    get_use_def_map: |sma| &mut sma.use_def_map,
};

impl CheckStateMachineUses {
    /// Analyze a state machine definition
    pub fn def_state_machine(
        sma: &mut StateMachineAnalysis,
        node: &fpp_ast::DefStateMachine,
    ) -> SmResult {
        StateMachineAnalysisVisitor::def_state_machine(&CheckStateMachineUses, sma, node)
    }

    /// Report the error of a use that does not resolve. The error is blocking:
    /// the later passes read each use out of the use-def map.
    fn check_use(sma: &mut StateMachineAnalysis, result: SemanticResult) -> SmResult {
        if let Err(err) = result {
            err.emit();
            sma.blocking_error = true;
        }
        ControlFlow::Continue(())
    }
}

impl StateMachineUseAnalyzer for CheckStateMachineUses {
    fn action_use(&self, sma: &mut StateMachineAnalysis, node: &Ident, _name: &str) -> SmResult {
        let result = HELPERS.visit_ident_node(StateMachineNameGroup::Action, sma, node, false);
        Self::check_use(sma, result)
    }

    fn guard_use(&self, sma: &mut StateMachineAnalysis, node: &Ident, _name: &str) -> SmResult {
        let result = HELPERS.visit_ident_node(StateMachineNameGroup::Guard, sma, node, false);
        Self::check_use(sma, result)
    }

    fn signal_use(&self, sma: &mut StateMachineAnalysis, node: &Ident, _name: &str) -> SmResult {
        let result = HELPERS.visit_ident_node(StateMachineNameGroup::Signal, sma, node, false);
        Self::check_use(sma, result)
    }

    fn state_or_choice_use(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &QualIdent,
        _name: QualifiedName,
    ) -> SmResult {
        let result = HELPERS.visit_qual_ident_node(StateMachineNameGroup::State, sma, node);
        Self::check_use(sma, result)
    }
}

impl StateMachineAnalysisVisitor for CheckStateMachineUses {
    state_machine_use_analyzer_hooks!();

    fn def_state(&self, sma: &mut StateMachineAnalysis, node: &DefState) -> SmResult {
        let symbol = StateMachineSymbol::State(Arc::new(node.clone()));
        let scope = sma.symbol_scope_map.get(&symbol).unwrap().clone();
        sma.nested_scope.push(scope);
        state_analyzer::def_state(self, sma, node)?;
        sma.nested_scope.pop();
        ControlFlow::Continue(())
    }
}
