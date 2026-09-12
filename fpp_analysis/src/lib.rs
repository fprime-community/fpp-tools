mod analysis;
mod errors;
pub mod used_symbols;
pub use used_symbols::UsedSymbols;

use crate::passes::{
    BuildSpecLocMap, CheckComponentDefs, CheckComponentInstanceDefs, CheckDictionaryDefs,
    CheckExprTypes, CheckFrameworkConstantValues, CheckFrameworkDefs, CheckInterfaceDefs,
    CheckPortDefs, CheckSpecLocs, CheckStateMachineDefs, CheckSystemDefs, CheckTopologyDefs,
    CheckTopologyInstances, CheckTypeUses, CheckUseDefCycles, CheckUses, ConstructDictionaryMap,
    ConstructImpliedUseMap, EnterSymbols, EvalConstantExprs, EvalImpliedEnumConsts,
    FinalizeTypeDefs,
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

    mod construct_dictionary_map;
    pub use construct_dictionary_map::*;

    pub(crate) mod check_spec_locs;
    pub use check_spec_locs::*;

    mod build_spec_loc_map;
    pub use build_spec_loc_map::*;

    mod check_system_defs;
    pub use check_system_defs::*;

    pub mod compute_dependencies;
    pub use compute_dependencies::*;
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

    mod tlm_channel_identifier;
    pub use tlm_channel_identifier::*;

    mod tlm_packet;
    pub use tlm_packet::*;

    pub mod tlm_packet_set;
    pub use tlm_packet_set::*;

    mod dictionary;
    pub use dictionary::*;

    pub mod construct_dictionary;
    pub use construct_dictionary::*;

    pub mod resolve_topology;
    pub use resolve_topology::*;

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

    pub mod state_machine;
    pub use state_machine::*;

    pub mod interned_def;
    pub use interned_def::*;

    mod generic_name_symbol_map;
    mod generic_nested_scope;
    mod generic_scope;

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
    let result =
        fpp_parser::ResolveIncludes::new(reader).visit_trans_unit(&mut a.include_context_map, ast);
    a.included_file_set = a
        .include_context_map
        .keys()
        .map(|f| fpp_core::File::from_string(&f.uri()))
        .collect();
    result
}

/// Check the semantics of a list of translation units.
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
    ConstructDictionaryMap::construct(a);
    CheckSystemDefs.visit_trans_units(a, ast.iter().cloned())?;

    ControlFlow::Continue(())
}
