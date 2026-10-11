use crate::run_test;

#[test]
fn duplicate_param() {
    run_test("port/duplicate_param")
}

#[test]
fn ok() {
    run_test("port/ok")
}

// Rust-only tests (no Scala counterpart)

/// A ref parameter with the name of an earlier parameter
#[test]
fn duplicate_ref_param() {
    run_test("port/duplicate_ref_param")
}
