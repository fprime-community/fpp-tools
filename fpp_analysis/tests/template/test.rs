use crate::{run_test, run_test_unconnected, run_test_with};

#[test]
fn alias_array_param_ok() {
    run_test("template/alias_array_param_ok")
}

#[test]
fn alias_param_arith_ok() {
    run_test("template/alias_param_arith_ok")
}

#[test]
fn alias_param_binop_shift_ok() {
    run_test("template/alias_param_binop_shift_ok")
}

#[test]
fn alias_param_dictionary_constant_error() {
    run_test("template/alias_param_dictionary_constant_error")
}

#[test]
fn alias_param_neg_float_shift_amount_error() {
    run_test("template/alias_param_neg_float_shift_amount_error")
}

#[test]
fn alias_param_neg_float_shift_error() {
    run_test("template/alias_param_neg_float_shift_error")
}

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
fn array_params_common_type_error() {
    run_test("template/array_params_common_type_error")
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
fn constant_param_string_size_order() {
    run_test("template/constant_param_string_size_order")
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
fn expand_absolute_global_template() {
    run_test("template/expand_absolute_global_template")
}

#[test]
fn expand_absolute_ok() {
    run_test("template/expand_absolute_ok")
}

#[test]
fn expand_absolute_qualified_ok() {
    run_test("template/expand_absolute_qualified_ok")
}

#[test]
fn expand_absolute_undefined() {
    run_test("template/expand_absolute_undefined")
}

#[test]
fn expand_before_invalid_composition() {
    run_test("template/expand_before_invalid_composition")
}

#[test]
fn expand_before_invalid_nested() {
    run_test("template/expand_before_invalid_nested")
}

#[test]
fn expand_in_include() {
    run_test("template/expand_in_include")
}

#[test]
fn expand_not_a_template() {
    run_test("template/expand_not_a_template")
}

#[test]
fn expand_not_a_template_args() {
    run_test("template/expand_not_a_template_args")
}

#[test]
fn expand_not_a_template_qualified() {
    run_test("template/expand_not_a_template_qualified")
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
fn interfaces_arg_not_instance() {
    run_test("template/interfaces_arg_not_instance")
}

#[test]
fn interfaces_arg_not_instance_unused() {
    run_test("template/interfaces_arg_not_instance_unused")
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
fn interfaces_param_not_interface() {
    run_test("template/interfaces_param_not_interface")
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
fn merge_group_order() {
    run_test("template/merge_group_order")
}

#[test]
fn merge_order() {
    run_test("template/merge_order")
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
fn struct_parameter_anon_array_promote() {
    run_test("template/struct_parameter_anon_array_promote")
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
fn struct_parameter_alias_default_ok() {
    run_test("template/struct_parameter_alias_default_ok")
}

#[test]
fn struct_parameter_array_member_default_ok() {
    run_test("template/struct_parameter_array_member_default_ok")
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

#[test]
fn constant_param_array_default_size_error() {
    run_test("template/constant_param_array_default_size_error")
}

#[test]
fn type_arg_array_sizeof() {
    run_test("template/type_arg_array_sizeof")
}

#[test]
fn type_arg_array_struct_member_sizeof() {
    run_test("template/type_arg_array_struct_member_sizeof")
}

#[test]
fn type_arg_array_alias_sizeof() {
    run_test("template/type_arg_array_alias_sizeof")
}

#[test]
fn type_arg_array_default() {
    run_test("template/type_arg_array_default")
}

#[test]
fn type_arg_array_framework_constant() {
    run_test("template/type_arg_array_framework_constant")
}

#[test]
fn constant_param_type_use_order() {
    run_test("template/constant_param_type_use_order")
}

#[test]
fn type_arg_use_order() {
    run_test("template/type_arg_use_order")
}

#[test]
fn type_arg_string_size_negative() {
    run_test("template/type_arg_string_size_negative")
}

#[test]
fn type_arg_string_size_sizeof() {
    run_test("template/type_arg_string_size_sizeof")
}

#[test]
fn expanded_topology_order() {
    run_test("template/expanded_topology_order")
}

#[test]
fn expanded_topology_import_order() {
    run_test("template/expanded_topology_import_order")
}

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
