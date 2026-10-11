use crate::run_test;

#[test]
fn bad_priority() {
    run_test("state_machine_instance/bad_priority")
}

#[test]
fn inside_active() {
    run_test("state_machine_instance/inside_active")
}

#[test]
fn inside_passive() {
    run_test("state_machine_instance/inside_passive")
}

#[test]
fn ok() {
    run_test("state_machine_instance/ok")
}

#[test]
fn outside_active() {
    run_test("state_machine_instance/outside_active")
}

#[test]
fn outside_passive() {
    run_test("state_machine_instance/outside_passive")
}

#[test]
fn undef_state_machine() {
    run_test("state_machine_instance/undef_state_machine")
}
