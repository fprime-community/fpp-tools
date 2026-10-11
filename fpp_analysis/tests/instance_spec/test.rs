use crate::run_test;

#[test]
fn undef_instance() {
    run_test("instance_spec/undef_instance")
}

#[test]
fn topology_instance() {
    run_test("instance_spec/topology_instance")
}

#[test]
fn ok() {
    run_test("instance_spec/ok")
}

#[test]
fn duplicate_instance() {
    run_test("instance_spec/duplicate_instance")
}

#[test]
fn shadowed_error() {
    run_test("instance_spec/shadowed_error")
}

#[test]
fn shadowed_ok() {
    run_test("instance_spec/shadowed_ok")
}
