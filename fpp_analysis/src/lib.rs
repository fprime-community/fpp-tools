mod analysis;
mod errors;

use crate::passes::{
    BuildSpecLocMap, CheckComponentDefs, CheckComponentInstanceDefs, CheckDictionaryDefs,
    CheckExprTypes, CheckFrameworkConstantValues, CheckFrameworkDefs, CheckInterfaceDefs,
    CheckPortDefs, CheckSpecLocs, CheckStateMachineDefs, CheckSystemDefs, CheckTopologyDefs,
    CheckTopologyInstances, CheckTypeUses, CheckUseDefCycles, CheckUses, ConstructImpliedUseMap,
    EnterSymbols, EvalConstantExprs, EvalImpliedEnumConsts, FinalizeTypeDefs,
};
pub use analysis::*;
use fpp_ast::{MutVisitor, Visitor};
use fpp_core::FileReader;
use std::ops::ControlFlow;

pub mod analyzers {
    pub mod analyzer;
    pub use analyzer::*;

    pub mod basic_use_analyzer;
    pub use basic_use_analyzer::*;

    pub mod nested_analyzer;
    pub use nested_analyzer::*;

    pub mod use_analyzer;
    pub use use_analyzer::*;

    pub mod state_machine;
}

pub mod passes {
    mod enter_symbols;
    pub use enter_symbols::*;

    mod check_state_machine_defs;
    pub use check_state_machine_defs::*;

    mod check_dictionary_defs;
    pub use check_dictionary_defs::*;

    mod construct_implied_use_map;
    pub use construct_implied_use_map::*;

    pub mod state_machine;

    mod check_uses;
    pub use check_uses::*;

    mod check_use_def_cycles;
    pub use check_use_def_cycles::*;

    mod check_type_uses;
    pub use check_type_uses::*;

    mod check_expr_types;
    pub use check_expr_types::*;

    mod eval_implied_enum_consts;
    pub use eval_implied_enum_consts::*;

    mod eval_constant_exprs;
    pub use eval_constant_exprs::*;

    mod finalize_type_defs;
    pub use finalize_type_defs::*;

    mod check_port_defs;
    pub use check_port_defs::*;

    mod check_framework_defs;
    pub use check_framework_defs::*;

    mod check_framework_constant_values;
    pub use check_framework_constant_values::*;

    mod check_interface_defs;
    pub use check_interface_defs::*;

    mod check_component_defs;
    pub use check_component_defs::*;

    mod check_component_instance_defs;
    pub use check_component_instance_defs::*;

    mod check_topology_instances;
    pub use check_topology_instances::*;

    mod check_topology_defs;
    pub use check_topology_defs::*;

    pub(crate) mod check_spec_locs;
    pub use check_spec_locs::*;

    mod build_spec_loc_map;
    pub use build_spec_loc_map::*;

    mod check_system_defs;
    pub use check_system_defs::*;
}

pub mod transform {
    mod add_state_enums;
    pub use add_state_enums::*;
}
pub use transform::add_state_enums;

pub mod semantics {
    mod symbol;
    pub use symbol::*;

    mod name;
    pub use name::*;

    mod implied_use;
    pub use implied_use::*;

    mod framework_definitions;
    pub use framework_definitions::*;

    mod interface;
    pub use interface::*;

    mod component;
    pub use component::*;

    mod component_instance;
    pub use component_instance::*;

    mod topology;
    pub use topology::*;

    mod system;
    pub use system::*;

    pub(crate) mod resolve_topology;

    mod connection;
    pub use connection::*;

    mod scope;
    pub use scope::*;

    mod name_groups;
    pub use name_groups::*;

    mod use_def_matching;
    pub use use_def_matching::*;

    mod types;
    pub use types::*;

    mod value;
    pub use value::*;

    mod format;
    pub use format::*;

    mod generic_name_symbol_map;
    mod generic_nested_scope;
    mod generic_scope;

    pub mod state_machine;

    #[cfg(test)]
    mod format_spec;
    #[cfg(test)]
    pub(crate) mod test_helpers;
    #[cfg(test)]
    mod type_spec;
    #[cfg(test)]
    mod value_spec;
}

pub fn resolve_includes<Reader: FileReader>(
    a: &mut Analysis,
    reader: Reader,
    ast: &mut fpp_ast::TransUnit,
) -> ControlFlow<()> {
    fpp_parser::ResolveIncludes::new(reader).visit_trans_unit(&mut a.include_context_map, ast)
}

/// Check the semantics of a list of translation units.
///
/// The passes run in the order below. A few passes are intentionally absent
/// (see docs/analysis-work-to-go.md):
///   - template resolution and template interface-arg checking: not
///     implemented.
///   - constant-expr finalization: folded into `CheckExprTypes` /
///     `EvalConstantExprs`.
///   - dictionary-map construction: codegen-support only, deferred.
pub fn check_semantics(a: &mut Analysis, ast: Vec<&fpp_ast::TransUnit>) -> ControlFlow<()> {
    EnterSymbols.visit_trans_units(a, ast.iter().cloned())?;
    ConstructImpliedUseMap.visit_trans_units(a, ast.iter().cloned())?;
    CheckUses::new().visit_trans_units(a, ast.iter().cloned())?;
    CheckUseDefCycles::new().visit_trans_units(a, ast.iter().cloned())?;
    CheckTypeUses::new().visit_trans_units(a, ast.iter().cloned())?;
    CheckExprTypes::new().visit_trans_units(a, ast.iter().cloned())?;
    CheckFrameworkDefs.visit_trans_units(a, ast.iter().cloned())?;
    EvalImpliedEnumConsts::new().visit_trans_units(a, ast.iter().cloned())?;
    EvalConstantExprs::new().visit_trans_units(a, ast.iter().cloned())?;
    FinalizeTypeDefs::new().visit_trans_units(a, ast.iter().cloned())?;
    CheckFrameworkConstantValues.check(a);
    CheckPortDefs.visit_trans_units(a, ast.iter().cloned())?;
    CheckInterfaceDefs.visit_trans_units(a, ast.iter().cloned())?;
    CheckComponentDefs.visit_trans_units(a, ast.iter().cloned())?;
    CheckComponentInstanceDefs.visit_trans_units(a, ast.iter().cloned())?;
    CheckComponentInstanceDefs::check_id_ranges(a);
    CheckStateMachineDefs.visit_trans_units(a, ast.iter().cloned())?;
    CheckTopologyInstances.visit_trans_units(a, ast.iter().cloned())?;
    CheckTopologyDefs.resolve_all(a);
    BuildSpecLocMap.visit_trans_units(a, ast.iter().cloned())?;
    CheckSpecLocs.visit_trans_units(a, ast.iter().cloned())?;
    CheckDictionaryDefs.visit_trans_units(a, ast.iter().cloned())?;
    CheckSystemDefs.visit_trans_units(a, ast.iter().cloned())?;

    ControlFlow::Continue(())
}
