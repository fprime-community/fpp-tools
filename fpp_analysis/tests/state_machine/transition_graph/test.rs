use crate::run_test;

#[test]
fn choice_cycle() {
    run_test("state_machine/transition_graph/choice_cycle")
}

#[test]
fn choice_cycle_ok() {
    run_test("state_machine/transition_graph/choice_cycle_ok")
}

#[test]
fn cycle_ok() {
    run_test("state_machine/transition_graph/cycle_ok")
}

#[test]
fn shadowed_state_error() {
    run_test("state_machine/transition_graph/shadowed_state_error")
}

#[test]
fn shadowed_state_ok() {
    run_test("state_machine/transition_graph/shadowed_state_ok")
}

#[test]
fn unreachable_choice() {
    run_test("state_machine/transition_graph/unreachable_choice")
}

#[test]
fn unreachable_state() {
    run_test("state_machine/transition_graph/unreachable_state")
}

// Rust-only tests (no Scala counterpart)

/// A choice cycle is reported before a type mismatch at a choice
#[test]
fn cycle_and_types() {
    run_test("state_machine/transition_graph/cycle_and_types")
}

/// The first of two choice cycles is reported
#[test]
fn two_choice_cycles() {
    run_test("state_machine/transition_graph/two_choice_cycles")
}

/// A choice cycle reached through a side branch
#[test]
fn choice_cycle_side_branch() {
    run_test("state_machine/transition_graph/choice_cycle_side_branch")
}
