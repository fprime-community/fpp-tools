use crate::run_test;

#[test]
fn nested_ok() {
    run_test("state_machine_defs/nested_ok")
}

#[test]
fn nested_type_undef() {
    run_test("state_machine_defs/nested_type_undef")
}

#[test]
fn nested_redef() {
    run_test("state_machine_defs/nested_redef")
}

#[test]
fn implied_state_enum_component_named_like_module_ok() {
    run_test("state_machine_defs/implied_state_enum_component_named_like_module_ok")
}

#[test]
fn implied_state_enum_module_named_like_module_ok() {
    run_test("state_machine_defs/implied_state_enum_module_named_like_module_ok")
}

#[test]
fn include_state_members_ok() {
    run_test("state_machine_defs/include_state_members_ok")
}

#[test]
fn include_state_machine_members_ok() {
    run_test("state_machine_defs/include_state_machine_members_ok")
}

#[test]
fn include_unreachable_state() {
    run_test("state_machine_defs/include_unreachable_state")
}
