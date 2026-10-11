use crate::run_test;

// Rust-only tests (no Scala counterpart)

/// Definitions nested in a state machine
#[test]
fn nested_ok() {
    run_test("state_machine_defs/nested_ok")
}

/// An undefined type in a signal of a state machine
#[test]
fn nested_type_undef() {
    run_test("state_machine_defs/nested_type_undef")
}

/// A redefinition inside a state machine
#[test]
fn nested_redef() {
    run_test("state_machine_defs/nested_redef")
}

/// The implied state enum of a state machine in a component named like its module
#[test]
fn implied_state_enum_component_named_like_module_ok() {
    run_test("state_machine_defs/implied_state_enum_component_named_like_module_ok")
}

/// The implied state enum of a state machine next to a module named like its module
#[test]
fn implied_state_enum_module_named_like_module_ok() {
    run_test("state_machine_defs/implied_state_enum_module_named_like_module_ok")
}

/// Members included into a state are members of that state
#[test]
fn include_state_members_ok() {
    run_test("state_machine_defs/include_state_members_ok")
}

/// Members included into a state machine are members of that state machine
#[test]
fn include_state_machine_members_ok() {
    run_test("state_machine_defs/include_state_machine_members_ok")
}

/// A state machine is checked with its included members
#[test]
fn include_unreachable_state() {
    run_test("state_machine_defs/include_unreachable_state")
}
