use crate::run_test;

#[test]
fn dp_state_not_enum() {
    run_test("framework_defs/dp_state_not_enum")
}

#[test]
fn fw_assert_arg_type_not_integer() {
    run_test("framework_defs/fw_assert_arg_type_not_integer")
}

#[test]
fn fw_event_id_type_not_alias_type() {
    run_test("framework_defs/fw_event_id_type_not_alias_type")
}

#[test]
fn fw_index_type_not_signed() {
    run_test("framework_defs/fw_index_type_not_signed")
}

#[test]
fn fw_log_string_max_size_not_integer() {
    run_test("framework_defs/fw_log_string_max_size_not_integer")
}

#[test]
fn fw_opcode_type_not_alias_type() {
    run_test("framework_defs/fw_opcode_type_not_alias_type")
}

#[test]
fn fw_obj_simple_reg_buff_size_not_integer() {
    run_test("framework_defs/fw_obj_simple_reg_buff_size_not_integer")
}

#[test]
fn fw_size_type_not_unsigned() {
    run_test("framework_defs/fw_size_type_not_unsigned")
}

#[test]
fn invalid_nonnegative_integer_constant() {
    run_test("framework_defs/invalid_nonnegative_integer_constant")
}

#[test]
fn invalid_positive_integer_constant() {
    run_test("framework_defs/invalid_positive_integer_constant")
}

#[test]
fn invalid_string_size() {
    run_test("framework_defs/invalid_string_size")
}

#[test]
fn proc_type_not_module_qualified() {
    run_test("framework_defs/proc_type_not_module_qualified")
}

#[test]
fn user_data_size_not_integer() {
    run_test("framework_defs/user_data_size_not_integer")
}

#[test]
fn user_data_size_not_module_qualified() {
    run_test("framework_defs/user_data_size_not_module_qualified")
}

// Rust-only tests (no Scala counterpart)

/// Members of a state machine in module Fw are not framework definitions
#[test]
fn state_machine_member_ok() {
    run_test("framework_defs/state_machine_member_ok")
}

/// The first of two invalid framework constants is reported
#[test]
fn two_invalid_constants() {
    run_test("framework_defs/two_invalid_constants")
}
