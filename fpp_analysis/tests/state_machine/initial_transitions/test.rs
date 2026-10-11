use crate::run_test;

#[test]
fn choice_cycle() {
    run_test("state_machine/initial_transitions/choice_cycle")
}

#[test]
fn external_state_machine() {
    run_test("state_machine/initial_transitions/external_state_machine")
}

#[test]
fn no_substates() {
    run_test("state_machine/initial_transitions/no_substates")
}

#[test]
fn ok() {
    run_test("state_machine/initial_transitions/ok")
}

#[test]
fn sm_choice_bad_parent_else() {
    run_test("state_machine/initial_transitions/sm_choice_bad_parent_else")
}

#[test]
fn sm_choice_bad_parent_if() {
    run_test("state_machine/initial_transitions/sm_choice_bad_parent_if")
}

#[test]
fn sm_choice_ok() {
    run_test("state_machine/initial_transitions/sm_choice_ok")
}

#[test]
fn sm_mismatched_parents() {
    run_test("state_machine/initial_transitions/sm_mismatched_parents")
}

#[test]
fn sm_multiple_transitions() {
    run_test("state_machine/initial_transitions/sm_multiple_transitions")
}

#[test]
fn sm_no_transition() {
    run_test("state_machine/initial_transitions/sm_no_transition")
}

#[test]
fn state_choice_bad_parent_else() {
    run_test("state_machine/initial_transitions/state_choice_bad_parent_else")
}

#[test]
fn state_choice_bad_parent_if() {
    run_test("state_machine/initial_transitions/state_choice_bad_parent_if")
}

#[test]
fn state_mismatched_parents() {
    run_test("state_machine/initial_transitions/state_mismatched_parents")
}

#[test]
fn state_multiple_transitions() {
    run_test("state_machine/initial_transitions/state_multiple_transitions")
}

#[test]
fn state_no_transition() {
    run_test("state_machine/initial_transitions/state_no_transition")
}
