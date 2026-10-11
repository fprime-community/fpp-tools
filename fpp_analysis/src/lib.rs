//! Semantic analysis of FPP models.
//!
//! # Collections
//!
//! Where the Scala compiler keeps an analysis result in an immutable `Map` or
//! `Set` and the order in which the collection is iterated is observable (it
//! orders diagnostics, dictionary entries or any other output), this crate keeps
//! it in an insertion-ordered [`indexmap::IndexMap`] or [`indexmap::IndexSet`].
//! Scala's immutable `Map` and `Set` iterate in insertion order up to four
//! entries and in hash order beyond that, so the two orders agree exactly up to
//! four entries; beyond four, the order here is still the deterministic insertion
//! order, not Scala's hash order. A collection whose iteration order is not
//! observable may be a hash map or set.

mod analysis;
mod errors;
pub mod used_symbols;
pub use used_symbols::UsedSymbols;

pub use crate::passes::{CheckSemantics, ExpandSyntaxAndEnterSymbols};
pub use analysis::*;
use fpp_ast::MutVisitor;
use fpp_core::FileReader;
use std::ops::ControlFlow;

pub mod analyzers {
    pub mod analyzer;
    pub use analyzer::*;

    pub mod basic_use_analyzer;
    pub use basic_use_analyzer::*;

    pub mod component_analyzer;
    pub use component_analyzer::*;

    pub mod enum_analyzer;
    pub use enum_analyzer::*;

    pub mod interface_analyzer;
    pub use interface_analyzer::*;

    pub mod module_analyzer;
    pub use module_analyzer::*;

    pub mod state_machine_analyzer;
    pub use state_machine_analyzer::*;

    pub mod template_expand_analyzer;
    pub use template_expand_analyzer::*;

    pub mod tlm_packet_set_analyzer;
    pub use tlm_packet_set_analyzer::*;

    pub mod topology_analyzer;
    pub use topology_analyzer::*;

    pub mod type_expression_analyzer;
    pub use type_expression_analyzer::*;

    pub mod use_analyzer;
    pub use use_analyzer::*;

    pub mod state_machine;
}

pub mod passes {
    mod expand_syntax_and_enter_symbols;
    pub use expand_syntax_and_enter_symbols::*;

    mod check_semantics;
    pub use check_semantics::*;

    mod enter_module_symbols;

    mod enter_symbols;
    pub use enter_symbols::*;

    mod check_state_machine_defs;
    pub use check_state_machine_defs::*;

    mod check_dictionary_defs;
    pub use check_dictionary_defs::*;

    mod construct_implied_use_map;
    pub use construct_implied_use_map::*;

    pub mod state_machine;

    pub(crate) mod check_uses;
    pub use check_uses::*;

    mod check_uses_helpers;
    pub use check_uses_helpers::*;

    mod check_template_uses;
    pub use check_template_uses::*;

    mod check_template_interface_args;
    pub use check_template_interface_args::*;

    mod enter_template_expansion_symbols;
    pub use enter_template_expansion_symbols::*;

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

    mod finalize_constant_exprs;
    pub use finalize_constant_exprs::*;

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

    mod check_system_defs;
    pub use check_system_defs::*;

    pub mod compute_dependencies;
    pub use compute_dependencies::*;
}

pub mod transform {
    pub use fpp_parser::add_state_enums;
    pub use fpp_parser::{IncludeParentKind, ResolveIncludes};

    mod expand_templates;
    pub use expand_templates::*;
}
pub use transform::{ExpandTemplates, add_state_enums};

pub mod semantics {
    mod symbol;
    pub use symbol::*;

    mod name;
    pub use name::*;

    mod implied_use;
    pub use implied_use::*;

    mod framework_definitions;
    pub use framework_definitions::*;

    mod port_instance;
    pub use port_instance::*;

    mod port_interface;
    pub use port_interface::*;

    mod interface;
    pub use interface::*;

    mod command;
    pub use command::*;

    mod container;
    pub use container::*;

    mod event;
    pub use event::*;

    mod param;
    pub use param::*;

    mod record;
    pub use record::*;

    mod tlm_channel;
    pub use tlm_channel::*;

    mod state_machine_instance;
    pub use state_machine_instance::*;

    mod component;
    pub use component::*;

    mod init_specifier;
    pub use init_specifier::*;

    mod component_instance;
    pub use component_instance::*;

    mod topology;
    pub use topology::*;

    mod topology_port;
    pub use topology_port::*;

    mod connection_pattern;
    pub use connection_pattern::*;

    mod fpp_system;
    pub use fpp_system::*;

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

    mod connection;
    pub use connection::*;

    mod scope;
    pub use scope::*;

    mod nested_scope;
    pub use nested_scope::*;

    mod name_symbol_map;
    pub use name_symbol_map::*;

    mod template_expansion;
    pub use template_expansion::*;

    mod name_group;
    pub use name_group::*;

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

    mod generic_scope;
    pub(crate) use generic_scope::{GenericNameSymbolMap, GenericNestedScope, GenericScope};

    #[cfg(test)]
    mod format_spec;
    #[cfg(test)]
    pub(crate) mod test_helpers;
    #[cfg(test)]
    mod type_option_spec;
    #[cfg(test)]
    mod type_spec;
    #[cfg(test)]
    mod value_spec;
}

/// Resolve the include specifiers of a translation unit, reading the included
/// files with `reader`.
///
/// The files included so far are recorded in `a.include_context_map`, with
/// the kind of member list each one is spliced into, and in
/// `a.included_file_set`.
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

/// Enter the symbols of a model and expand its templates: the steps of
/// [`ExpandSyntaxAndEnterSymbols::tu_list`] that follow include resolution and
/// the synthesis of the state enums (see
/// [`ExpandSyntaxAndEnterSymbols::enter_symbols_and_expand_templates`]).
///
/// This is a lower-level entry point than [`CheckSemantics::tu_list`], for a
/// driver that resolves the includes and adds the state enums of each
/// translation unit itself ([`resolve_includes`], [`add_state_enums`]). It
/// runs before [`check_semantics`], on the same analysis, and it rewrites the
/// translation units, replacing each template expansion specifier with the
/// expanded template. A `Break` result means the caller must not go on to
/// [`check_semantics`].
pub fn resolve_templates(a: &mut Analysis, tul: &mut [fpp_ast::TransUnit]) -> ControlFlow<()> {
    ExpandSyntaxAndEnterSymbols::enter_symbols_and_expand_templates(a, tul)
}

/// Check the semantics of a list of translation units: the steps of
/// [`CheckSemantics::tu_list`] that follow
/// [`ExpandSyntaxAndEnterSymbols::tu_list`].
///
/// This is a lower-level entry point than [`CheckSemantics::tu_list`].
/// [`resolve_templates`] must have run on the same analysis first: it enters the
/// symbols of the model.
pub fn check_semantics(a: &mut Analysis, tul: Vec<&fpp_ast::TransUnit>) -> ControlFlow<()> {
    CheckSemantics::check_expanded_tu_list(a, &tul)
}
