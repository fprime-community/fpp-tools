use crate::{run_test, run_test_unconnected, run_test_with};

// The tests that call run_test_unconnected also check the unconnected port
// report. The report makes the instances and connections of an expanded
// topology observable.

#[test]
fn arg_kind_constant_param_instance_arg() {
    run_test("template/arg_kind_constant_param_instance_arg")
}

#[test]
fn arg_kind_constant_param_type_arg() {
    run_test("template/arg_kind_constant_param_type_arg")
}

#[test]
fn arg_kind_instance_param_constant_arg() {
    run_test("template/arg_kind_instance_param_constant_arg")
}

#[test]
fn arg_kind_instance_param_type_arg() {
    run_test("template/arg_kind_instance_param_type_arg")
}

#[test]
fn arg_kind_type_param_constant_arg() {
    run_test("template/arg_kind_type_param_constant_arg")
}

#[test]
fn arg_kind_type_param_instance_arg() {
    run_test("template/arg_kind_type_param_instance_arg")
}

#[test]
fn arity_no_params_ok() {
    run_test("template/arity_no_params_ok")
}

#[test]
fn arity_omitted_args() {
    run_test("template/arity_omitted_args")
}

#[test]
fn arity_too_few_args() {
    run_test("template/arity_too_few_args")
}

#[test]
fn arity_too_many_args() {
    run_test("template/arity_too_many_args")
}

#[test]
fn basic_constant() {
    run_test("template/basic_constant")
}

#[test]
fn bound_param_capture() {
    run_test("template/bound_param_capture")
}

#[test]
fn bound_param_capture_ok() {
    run_test("template/bound_param_capture_ok")
}

#[test]
fn chained_constants() {
    run_test("template/chained_constants")
}

#[test]
fn composition_invalid() {
    run_test("template/composition_invalid")
}

#[test]
fn constant_param_array_promote() {
    run_test("template/constant_param_array_promote")
}

#[test]
fn constant_param_array_promote_bad_subscript() {
    run_test("template/constant_param_array_promote_bad_subscript")
}

#[test]
fn constant_with_default() {
    run_test("template/constant_with_default")
}

#[test]
fn definition_point_scoping() {
    run_test("template/definition_point_scoping")
}

#[test]
fn definition_point_scoping_ok() {
    run_test("template/definition_point_scoping_ok")
}

#[test]
fn duplicate_def() {
    run_test("template/duplicate_def")
}

#[test]
fn import_two_expansions_ok() {
    run_test_unconnected("template/import_two_expansions_ok")
}

#[test]
fn interfaces() {
    run_test("template/interfaces")
}

#[test]
fn interfaces_arg_bad_signature() {
    run_test("template/interfaces_arg_bad_signature")
}

#[test]
fn interfaces_arg_conformance_ok() {
    run_test("template/interfaces_arg_conformance_ok")
}

#[test]
fn interfaces_arg_missing_port() {
    run_test("template/interfaces_arg_missing_port")
}

#[test]
fn interfaces_basic() {
    run_test("template/interfaces_basic")
}

#[test]
fn interfaces_invalid_port_instance() {
    run_test("template/interfaces_invalid_port_instance")
}

#[test]
fn interfaces_invalid_port_instance_name() {
    run_test("template/interfaces_invalid_port_instance_name")
}

#[test]
fn interfaces_invalid_port_instance_name_aliased() {
    run_test("template/interfaces_invalid_port_instance_name_aliased")
}

#[test]
fn interfaces_pattern_explicit_target_ok() {
    run_test_unconnected("template/interfaces_pattern_explicit_target_ok")
}

#[test]
fn interfaces_pattern_explicit_target_topology() {
    run_test("template/interfaces_pattern_explicit_target_topology")
}

#[test]
fn interfaces_topology_arg_deployment() {
    run_test("template/interfaces_topology_arg_deployment")
}

#[test]
fn interfaces_topology_arg_ok() {
    run_test_unconnected("template/interfaces_topology_arg_ok")
}

#[test]
fn interfaces_topology_arg_order_ok() {
    run_test_unconnected("template/interfaces_topology_arg_order_ok")
}

#[test]
fn interfaces_valid_port_instance_name_aliased() {
    run_test("template/interfaces_valid_port_instance_name_aliased")
}

#[test]
fn invalid_nested() {
    run_test("template/invalid_nested")
}

#[test]
fn module_merge_ok() {
    run_test("template/module_merge_ok")
}

#[test]
fn nested_scope_resolution() {
    run_test("template/nested_scope_resolution")
}

#[test]
fn nested_scope_resolution_ok() {
    run_test("template/nested_scope_resolution_ok")
}

#[test]
fn param_body_def_different_group_ok() {
    run_test("template/param_body_def_different_group_ok")
}

#[test]
fn param_conflict_body_def() {
    run_test("template/param_conflict_body_def")
}

#[test]
fn param_conflict_same_name_group() {
    run_test("template/param_conflict_same_name_group")
}

#[test]
fn param_same_name_different_groups_ok() {
    run_test("template/param_same_name_different_groups_ok")
}

#[test]
fn parameter_shadow_error() {
    run_test("template/parameter_shadow_error")
}

#[test]
fn parameter_shadow_ok() {
    run_test("template/parameter_shadow_ok")
}

#[test]
fn struct_parameter_alias_default_ok() {
    run_test("template/struct_parameter_alias_default_ok")
}

#[test]
fn struct_parameter_anon_array_promote() {
    run_test("template/struct_parameter_anon_array_promote")
}

#[test]
fn struct_parameter_array_member_default_ok() {
    run_test("template/struct_parameter_array_member_default_ok")
}

#[test]
fn struct_parameter_bad_arg() {
    run_test("template/struct_parameter_bad_arg")
}

#[test]
fn struct_parameter_bad_concat() {
    run_test("template/struct_parameter_bad_concat")
}

#[test]
fn struct_parameter_concat_primitives_promote() {
    run_test("template/struct_parameter_concat_primitives_promote")
}

#[test]
fn struct_parameter_nested_default_bad_arg() {
    run_test("template/struct_parameter_nested_default_bad_arg")
}

#[test]
fn struct_parameter_nested_default_ok() {
    run_test("template/struct_parameter_nested_default_ok")
}

#[test]
fn struct_parameter_ok() {
    run_test("template/struct_parameter_ok")
}

#[test]
fn struct_parameter_omitted_member_ok() {
    run_test("template/struct_parameter_omitted_member_ok")
}

#[test]
fn type_arg_string() {
    run_test("template/type_arg_string")
}

#[test]
fn type_param_implied_use() {
    run_test("template/type_param_implied_use")
}

#[test]
fn types() {
    run_test("template/types")
}

#[test]
fn undef_constant_param_type() {
    run_test("template/undef_constant_param_type")
}

#[test]
fn undef_interface_param_type() {
    run_test("template/undef_interface_param_type")
}

#[test]
fn use_before_expand() {
    run_test("template/use_before_expand")
}

#[test]
fn use_before_expand_ok() {
    run_test("template/use_before_expand_ok")
}

// Rust-only tests (no Scala counterpart)

/// An argument bound to a constant parameter of an alias of an array type
#[test]
fn alias_array_param_ok() {
    run_test("template/alias_array_param_ok")
}

/// Arithmetic on a value of an alias type has the numeric type of the alias
#[test]
fn alias_param_arith_ok() {
    run_test("template/alias_param_arith_ok")
}

/// An arithmetic expression whose common type is an alias of an integer type may be shifted
#[test]
fn alias_param_binop_shift_ok() {
    run_test("template/alias_param_binop_shift_ok")
}

/// A dictionary constant may not have an alias type
#[test]
fn alias_param_dictionary_constant_error() {
    run_test("template/alias_param_dictionary_constant_error")
}

/// A negated value of an alias of a float type is not a shift amount
#[test]
fn alias_param_neg_float_shift_amount_error() {
    run_test("template/alias_param_neg_float_shift_amount_error")
}

/// A negated value of an alias of a float type may not be shifted
#[test]
fn alias_param_neg_float_shift_error() {
    run_test("template/alias_param_neg_float_shift_error")
}

/// Two array parameters may not be added
#[test]
fn array_params_common_type_error() {
    run_test("template/array_params_common_type_error")
}

/// The string size of a constant parameter type is checked with the bound argument
#[test]
fn constant_param_string_size_order() {
    run_test("template/constant_param_string_size_order")
}

/// An absolute template name refers to the template in the global scope
#[test]
fn expand_absolute_global_template() {
    run_test("template/expand_absolute_global_template")
}

/// An absolute template name is not resolved in the enclosing module
#[test]
fn expand_absolute_ok() {
    run_test("template/expand_absolute_ok")
}

/// The qualifier of an absolute template name is resolved in the global scope
#[test]
fn expand_absolute_qualified_ok() {
    run_test("template/expand_absolute_qualified_ok")
}

/// An absolute template name is looked up in the global scope only
#[test]
fn expand_absolute_undefined() {
    run_test("template/expand_absolute_undefined")
}

/// An expansion specifier in a template body is reported at the first expansion
#[test]
fn expand_before_invalid_composition() {
    run_test("template/expand_before_invalid_composition")
}

/// A nested template in a template body is reported at the first expansion
#[test]
fn expand_before_invalid_nested() {
    run_test("template/expand_before_invalid_nested")
}

/// A diagnostic in an expansion reports where its included specifier is
#[test]
fn expand_in_include() {
    run_test("template/expand_in_include")
}

/// An expansion specifier that names a module
#[test]
fn expand_not_a_template() {
    run_test("template/expand_not_a_template")
}

/// An expansion specifier with arguments that names a module
#[test]
fn expand_not_a_template_args() {
    run_test("template/expand_not_a_template_args")
}

/// An expansion specifier that names a nested module
#[test]
fn expand_not_a_template_qualified() {
    run_test("template/expand_not_a_template_qualified")
}

/// A module is not an instance argument
#[test]
fn interfaces_arg_not_instance() {
    run_test("template/interfaces_arg_not_instance")
}

/// A module is not an instance argument, even for a parameter no topology uses
#[test]
fn interfaces_arg_not_instance_unused() {
    run_test("template/interfaces_arg_not_instance_unused")
}

/// A module is not the interface of an instance parameter
#[test]
fn interfaces_param_not_interface() {
    run_test("template/interfaces_param_not_interface")
}

/// The value name group of an expansion is merged before the type name group
#[test]
fn merge_group_order() {
    run_test("template/merge_group_order")
}

/// The symbols of an expansion are merged in the order the template enters them
#[test]
fn merge_order() {
    run_test("template/merge_order")
}

/// A constant argument of the wrong size as an array default
#[test]
fn constant_param_array_default_size_error() {
    run_test("template/constant_param_array_default_size_error")
}

/// The size of an array of a type argument
#[test]
fn type_arg_array_sizeof() {
    run_test("template/type_arg_array_sizeof")
}

/// The size of a struct with a member of a type argument
#[test]
fn type_arg_array_struct_member_sizeof() {
    run_test("template/type_arg_array_struct_member_sizeof")
}

/// The size of an alias of a type argument
#[test]
fn type_arg_array_alias_sizeof() {
    run_test("template/type_arg_array_alias_sizeof")
}

/// An array default checked against a size computed from a type argument
#[test]
fn type_arg_array_default() {
    run_test("template/type_arg_array_default")
}

/// A framework constant computed from the size of a type argument
#[test]
fn type_arg_array_framework_constant() {
    run_test("template/type_arg_array_framework_constant")
}

/// An array type used by a template parameter is checked first
#[test]
fn constant_param_type_use_order() {
    run_test("template/constant_param_type_use_order")
}

/// An array type used as a type argument is checked first
#[test]
fn type_arg_use_order() {
    run_test("template/type_arg_use_order")
}

/// A negative string size in a type argument
#[test]
fn type_arg_string_size_negative() {
    run_test("template/type_arg_string_size_negative")
}

/// The size of a string type argument
#[test]
fn type_arg_string_size_sizeof() {
    run_test("template/type_arg_string_size_sizeof")
}

/// A topology from an expansion is resolved at the expansion
#[test]
fn expanded_topology_order() {
    run_test("template/expanded_topology_order")
}

/// A topology from an expansion is resolved after the topologies before the expansion
#[test]
fn expanded_topology_import_order() {
    run_test("template/expanded_topology_import_order")
}

/// Instance arguments are checked in the order of the input files
#[test]
fn interface_arg_file_order() {
    run_test_with(
        &[
            "template/interface_arg_file_order_common",
            "template/interface_arg_file_order_z",
        ],
        "template/interface_arg_file_order_a",
    )
}
