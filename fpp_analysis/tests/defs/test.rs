use crate::run_test;

#[test]
fn ok() {
    run_test("defs/ok")
}

// Rust-only tests (no Scala counterpart)

/// An interface definition and a module defined in two parts
#[test]
fn ok_interface_module_reopen() {
    run_test("defs/ok_interface_module_reopen")
}
