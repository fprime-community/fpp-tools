use crate::run_test;

#[test]
fn ok() {
    run_test("defs/ok")
}

#[test]
fn ok_interface_module_reopen() {
    run_test("defs/ok_interface_module_reopen")
}
