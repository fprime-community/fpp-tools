use crate::analyzers::state_machine::{SmResult, StateMachineAnalysisVisitor};
use crate::semantics::state_machine::StateMachineAnalysis;
use fpp_ast::DefState;
use std::ops::ControlFlow;

/// Analyze state definition members
///
/// Push the state name onto the scope name list, visit the state members, and
/// restore the scope name list. This is the default `def_state` of every
/// [`StateMachineAnalysisVisitor`]; a pass that overrides `def_state` calls it
/// to visit the members.
pub fn def_state<V: StateMachineAnalysisVisitor + ?Sized>(
    v: &V,
    sma: &mut StateMachineAnalysis,
    node: &DefState,
) -> SmResult {
    let scope_name_list = sma.scope_name_list.clone();
    sma.scope_name_list.insert(0, node.name.data.clone());
    v.visit_state_members(sma, &node.members)?;
    sma.scope_name_list = scope_name_list;
    ControlFlow::Continue(())
}
