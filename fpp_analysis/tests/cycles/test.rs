use crate::run_test;

#[test]
fn struct_() {
    run_test("cycles/struct")
}

#[test]
fn alias() {
    run_test("cycles/alias")
}

#[test]
fn enum_constant() {
    run_test("cycles/enum_constant")
}

#[test]
fn constant_2() {
    run_test("cycles/constant_2")
}

#[test]
fn enum_() {
    run_test("cycles/enum")
}

#[test]
fn array() {
    run_test("cycles/array")
}

#[test]
fn constant_1() {
    run_test("cycles/constant_1")
}

#[test]
fn interface() {
    run_test("cycles/interface")
}

#[test]
fn topology() {
    run_test("cycles/topology")
}

#[test]
fn sizeof_string_1() {
    run_test("cycles/sizeof_string_1")
}

#[test]
fn sizeof_string_2() {
    run_test("cycles/sizeof_string_2")
}

#[test]
fn topology_implements() {
    run_test("cycles/topology_implements")
}
