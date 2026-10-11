use crate::run_test;

#[test]
fn constant_ok() {
    run_test("state_machine/constants/constant_ok")
}

#[test]
fn shadowed_error() {
    run_test("state_machine/constants/shadowed_error")
}

#[test]
fn shadowed_ok() {
    run_test("state_machine/constants/shadowed_ok")
}
