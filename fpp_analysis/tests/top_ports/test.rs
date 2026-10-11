use crate::run_test;

#[test]
fn basic() {
    run_test("top_ports/basic")
}

#[test]
fn implements() {
    run_test("top_ports/implements")
}

#[test]
fn implements_port_mismatch_1() {
    run_test("top_ports/implements_port_mismatch_1")
}

#[test]
fn implements_port_mismatch_2() {
    run_test("top_ports/implements_port_mismatch_2")
}

#[test]
fn implements_port_missing() {
    run_test("top_ports/implements_port_missing")
}

#[test]
fn interface_instance_not_member() {
    run_test("top_ports/interface_instance_not_member")
}

#[test]
fn internal_port() {
    run_test("top_ports/internal_port")
}

#[test]
fn nested() {
    run_test("top_ports/nested")
}

#[test]
fn out_to_out() {
    run_test("top_ports/out_to_out")
}

#[test]
fn top_to_top() {
    run_test("top_ports/top_to_top")
}

#[test]
fn unmatched_types() {
    run_test("top_ports/unmatched_types")
}

// Rust-only tests (no Scala counterpart)

/// An unmatched connection through a topology port alias
#[test]
fn unmatched_through_alias() {
    run_test("top_ports/unmatched_through_alias")
}

/// The first missing port of an implemented interface is reported
#[test]
fn implements_first_missing_port() {
    run_test("top_ports/implements_first_missing_port")
}

/// The missing ports of an implemented interface are checked in import order
#[test]
fn implements_missing_port_import_order() {
    run_test("top_ports/implements_missing_port_import_order")
}

/// The first missing port of an interface with more than four ports is reported
#[test]
fn implements_missing_port_beyond_four() {
    run_test("top_ports/implements_missing_port_beyond_four")
}

/// A topology port of a topology that is resolved later is an error
#[test]
fn unresolved_topology() {
    run_test("top_ports/unresolved_topology")
}

/// Topology ports are checked in definition order
#[test]
fn port_order() {
    run_test("top_ports/port_order")
}
