use crate::run_test;

#[test]
fn duplicate_param() {
    run_test("port/duplicate_param")
}

#[test]
fn ok() {
    run_test("port/ok")
}

#[test]
fn duplicate_ref_param() {
    run_test("port/duplicate_ref_param")
}
