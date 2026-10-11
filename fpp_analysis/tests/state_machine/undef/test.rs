use crate::run_test;

#[test]
fn action_error() {
    run_test("state_machine/undef/action_error")
}

#[test]
fn action_ok() {
    run_test("state_machine/undef/action_ok")
}

#[test]
fn choice_error() {
    run_test("state_machine/undef/choice_error")
}

#[test]
fn choice_ok() {
    run_test("state_machine/undef/choice_ok")
}

#[test]
fn constant_error() {
    run_test("state_machine/undef/constant_error")
}

#[test]
fn guard_error() {
    run_test("state_machine/undef/guard_error")
}

#[test]
fn guard_ok() {
    run_test("state_machine/undef/guard_ok")
}

#[test]
fn nested_action_error() {
    run_test("state_machine/undef/nested_action_error")
}

#[test]
fn nested_action_ok() {
    run_test("state_machine/undef/nested_action_ok")
}

#[test]
fn nested_choice_error() {
    run_test("state_machine/undef/nested_choice_error")
}

#[test]
fn nested_choice_ok() {
    run_test("state_machine/undef/nested_choice_ok")
}

#[test]
fn nested_guard_error() {
    run_test("state_machine/undef/nested_guard_error")
}

#[test]
fn nested_guard_ok() {
    run_test("state_machine/undef/nested_guard_ok")
}

#[test]
fn nested_state_error() {
    run_test("state_machine/undef/nested_state_error")
}

#[test]
fn nested_state_ok() {
    run_test("state_machine/undef/nested_state_ok")
}

#[test]
fn signal_error() {
    run_test("state_machine/undef/signal_error")
}

#[test]
fn signal_ok() {
    run_test("state_machine/undef/signal_ok")
}

#[test]
fn state_error() {
    run_test("state_machine/undef/state_error")
}

#[test]
fn state_ok() {
    run_test("state_machine/undef/state_ok")
}

// Rust-only tests (no Scala counterpart)

/// An undefined member in a qualified transition target
#[test]
fn qualified_target() {
    run_test("state_machine/undef/qualified_target")
}

/// Several undefined signal and action uses in one state
#[test]
fn multiple_uses() {
    run_test("state_machine/undef/multiple_uses")
}

/// An undefined initial transition target and undefined uses inside a state
#[test]
fn stress_multi() {
    run_test("state_machine/undef/stress_multi")
}

/// An absolute state name is looked up in the outermost scope only
#[test]
fn absolute_state_error() {
    run_test("state_machine/undef/absolute_state_error")
}

/// A choice is not a qualifier
#[test]
fn choice_as_qualifier() {
    run_test("state_machine/undef/choice_as_qualifier")
}
