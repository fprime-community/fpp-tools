use crate::run_test;

#[test]
fn alias() {
    run_test("cycle/alias")
}

#[test]
fn array() {
    run_test("cycle/array")
}

#[test]
fn constant_1() {
    run_test("cycle/constant_1")
}

#[test]
fn constant_2() {
    run_test("cycle/constant_2")
}

#[test]
fn r#enum() {
    run_test("cycle/enum")
}

#[test]
fn enum_constant() {
    run_test("cycle/enum_constant")
}

#[test]
fn sizeof_string_1() {
    run_test("cycle/sizeof_string_1")
}

#[test]
fn sizeof_string_2() {
    run_test("cycle/sizeof_string_2")
}

#[test]
fn r#struct() {
    run_test("cycle/struct")
}

// Rust-only tests (no Scala counterpart)

/// A cycle of interface imports
#[test]
fn interface() {
    run_test("cycle/interface")
}

/// A cycle of topology instances
#[test]
fn topology() {
    run_test("cycle/topology")
}

/// A cycle of interface imports reached from an implements clause
#[test]
fn topology_implements() {
    run_test("cycle/topology_implements")
}

/// A cycle reached through a use outside the cycle
#[test]
fn tail() {
    run_test("cycle/tail")
}

/// A cycle through a template constant argument
#[test]
fn template_constant_arg() {
    run_test("cycle/template_constant_arg")
}

/// A cycle through a template type argument
#[test]
fn template_type_arg() {
    run_test("cycle/template_type_arg")
}

/// A cycle through a template interface argument
#[test]
fn template_interface_arg() {
    run_test("cycle/template_interface_arg")
}

/// A cycle through the implied use of a string type
#[test]
fn implied_type_use() {
    run_test("cycle/implied_type_use")
}
