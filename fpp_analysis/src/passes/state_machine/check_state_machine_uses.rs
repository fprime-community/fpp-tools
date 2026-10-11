use crate::analyzers::state_machine::{
    SmResult, StateMachineAnalysisVisitor, StateMachineUseAnalyzer, state_analyzer,
};
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::SymbolInterface;
use crate::semantics::state_machine::{
    StateMachineAnalysis, StateMachineNameGroup, StateMachineSymbol,
};
use fpp_ast::{
    AstNode, DefChoice, DefState, Ident, QualIdent, SpecInitialTransition, SpecStateEntry,
    SpecStateExit, SpecStateTransition,
};
use fpp_core::Spanned;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Match uses to their definitions
pub struct CheckStateMachineUses;

impl CheckStateMachineUses {
    /// Get the symbol for a name from the environment
    fn get_symbol_for_name(
        ng: StateMachineNameGroup,
        mapping: impl FnOnce(&str) -> Option<StateMachineSymbol>,
        id: fpp_core::Node,
        name: &str,
    ) -> SemanticResult<StateMachineSymbol> {
        mapping(name).ok_or_else(|| SemanticError::UndefinedSymbol {
            ng: ng.to_string(),
            name: name.to_string(),
            loc: id.span(),
        })
    }

    /// Visit an identifier node and check a use
    fn visit_ident_node(
        &self,
        sma: &mut StateMachineAnalysis,
        ng: StateMachineNameGroup,
        node: &Ident,
    ) -> SmResult {
        self.visit_unqualified_name(sma, ng, node.id(), node, false)
    }

    /// Visit a qualified identifier node and check a use
    fn visit_qual_ident_node(
        &self,
        sma: &mut StateMachineAnalysis,
        ng: StateMachineNameGroup,
        node: &QualIdent,
    ) -> SmResult {
        match node {
            QualIdent::Unqualified(unqualified) => self.visit_unqualified_name(
                sma,
                ng,
                node.id(),
                &unqualified.name,
                unqualified.is_absolute,
            ),
            QualIdent::Qualified(qualified) => {
                self.visit_qualified_name(sma, ng, node.id(), &qualified.qualifier, &qualified.name)
            }
        }
    }

    fn visit_unqualified_name(
        &self,
        sma: &mut StateMachineAnalysis,
        ng: StateMachineNameGroup,
        id: fpp_core::Node,
        name: &Ident,
        is_absolute: bool,
    ) -> SmResult {
        let mapping = |name: &str| sma.nested_scope.get(is_absolute, ng, name);
        match Self::get_symbol_for_name(ng, mapping, id, &name.data) {
            Ok(symbol) => {
                sma.use_def_map.insert(id, symbol);
            }
            Err(err) => {
                // Blocking: later passes read this use out of `use_def_map`.
                err.emit();
                sma.blocking_error = true;
            }
        }
        ControlFlow::Continue(())
    }

    fn visit_qualified_name(
        &self,
        sma: &mut StateMachineAnalysis,
        ng: StateMachineNameGroup,
        id: fpp_core::Node,
        qualifier: &QualIdent,
        name: &Ident,
    ) -> SmResult {
        self.visit_qual_ident_node(sma, ng, qualifier)?;
        // If the qualifier itself did not resolve, its (blocking) error was
        // already emitted; we cannot resolve the member, so stop here.
        let Some(symbol) = sma.use_def_map.get(&qualifier.id()).cloned() else {
            return ControlFlow::Continue(());
        };
        let Some(scope) = sma.symbol_scope_map.get(&symbol) else {
            SemanticError::InvalidSymbol {
                symbol_name: symbol.get_unqualified_name().to_string(),
                loc: id.span(),
                msg: "not a qualifier".to_string(),
                def_loc: symbol.get_loc(),
            }
            .emit();
            sma.blocking_error = true;
            return ControlFlow::Continue(());
        };
        let mapping = |name: &str| scope.get(ng, name);
        match Self::get_symbol_for_name(ng, mapping, name.id(), &name.data) {
            Ok(symbol) => {
                sma.use_def_map.insert(id, symbol);
            }
            Err(err) => {
                err.emit();
                sma.blocking_error = true;
            }
        }
        ControlFlow::Continue(())
    }

    /// Analyze a state machine definition
    pub fn def_state_machine(
        sma: &mut StateMachineAnalysis,
        node: &fpp_ast::DefStateMachine,
    ) -> SmResult {
        StateMachineAnalysisVisitor::def_state_machine(&CheckStateMachineUses, sma, node)
    }
}

impl StateMachineUseAnalyzer for CheckStateMachineUses {
    fn action_use(&self, sma: &mut StateMachineAnalysis, node: &Ident) -> SmResult {
        self.visit_ident_node(sma, StateMachineNameGroup::Action, node)
    }

    fn guard_use(&self, sma: &mut StateMachineAnalysis, node: &Ident) -> SmResult {
        self.visit_ident_node(sma, StateMachineNameGroup::Guard, node)
    }

    fn signal_use(&self, sma: &mut StateMachineAnalysis, node: &Ident) -> SmResult {
        self.visit_ident_node(sma, StateMachineNameGroup::Signal, node)
    }

    fn state_or_choice_use(&self, sma: &mut StateMachineAnalysis, node: &QualIdent) -> SmResult {
        self.visit_qual_ident_node(sma, StateMachineNameGroup::State, node)
    }
}

impl StateMachineAnalysisVisitor for CheckStateMachineUses {
    fn def_choice(&self, sma: &mut StateMachineAnalysis, node: &DefChoice) -> SmResult {
        self.def_choice_uses(sma, node)
    }

    fn spec_state_entry(&self, sma: &mut StateMachineAnalysis, node: &SpecStateEntry) -> SmResult {
        self.spec_state_entry_uses(sma, node)
    }

    fn spec_state_exit(&self, sma: &mut StateMachineAnalysis, node: &SpecStateExit) -> SmResult {
        self.spec_state_exit_uses(sma, node)
    }

    fn spec_initial_transition(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &SpecInitialTransition,
    ) -> SmResult {
        self.spec_initial_transition_uses(sma, node)
    }

    fn spec_state_transition(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &SpecStateTransition,
    ) -> SmResult {
        self.spec_state_transition_uses(sma, node)
    }

    fn def_state(&self, sma: &mut StateMachineAnalysis, node: &DefState) -> SmResult {
        let symbol = StateMachineSymbol::State(Arc::new(node.clone()));
        let scope = sma.symbol_scope_map.get(&symbol).unwrap().clone();
        sma.nested_scope.push(scope);
        state_analyzer::def_state(self, sma, node)?;
        sma.nested_scope.pop();
        ControlFlow::Continue(())
    }
}
