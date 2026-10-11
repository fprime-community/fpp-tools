use crate::{run_test, run_test_with};

#[test]
fn instance_not_in_topology() {
    run_test("connection_direct/instance_not_in_topology")
}

#[test]
fn internal_port() {
    run_test("connection_direct/internal_port")
}

#[test]
fn invalid_directions() {
    run_test("connection_direct/invalid_directions")
}

#[test]
fn invalid_port_instance() {
    run_test("connection_direct/invalid_port_instance")
}

#[test]
fn invalid_port_number() {
    run_test("connection_direct/invalid_port_number")
}

#[test]
fn invalid_unmatched_component_connection() {
    run_test("connection_direct/invalid_unmatched_component_connection")
}

#[test]
fn invalid_unmatched_topology_connection() {
    run_test("connection_direct/invalid_unmatched_topology_connection")
}

#[test]
fn mismatched_port_types() {
    run_test("connection_direct/mismatched_port_types")
}

#[test]
fn ok() {
    run_test("connection_direct/ok")
}

#[test]
fn serial_to_typed_with_return() {
    run_test("connection_direct/serial_to_typed_with_return")
}

#[test]
fn shadowed_instance_error() {
    run_test("connection_direct/shadowed_instance_error")
}

#[test]
fn shadowed_instance_ok() {
    run_test("connection_direct/shadowed_instance_ok")
}

#[test]
fn shadowed_port_error() {
    run_test("connection_direct/shadowed_port_error")
}

#[test]
fn shadowed_port_ok() {
    run_test("connection_direct/shadowed_port_ok")
}

#[test]
fn typed_to_serial_with_return() {
    run_test("connection_direct/typed_to_serial_with_return")
}

#[test]
fn undef_instance() {
    run_test("connection_direct/undef_instance")
}

// Rust-only tests (no Scala counterpart)

/// A connection to a topology that is resolved later is an error
#[test]
fn unresolved_topology() {
    run_test("connection_direct/unresolved_topology")
}

/// A connection error in the including file comes before one in the included file
#[test]
fn include_first_error() {
    run_test("connection_direct/include_first_error")
}

/// Connection errors are reported in the order of the input files
#[test]
fn file_order() {
    run_test_with(
        &[
            "connection_direct/file_order_common",
            "connection_direct/file_order_z",
        ],
        "connection_direct/file_order_a",
    )
}

/// Instances that are not topology members are reported in connection order
#[test]
fn not_member_order() {
    run_test("connection_direct/not_member_order")
}

/// Connection graphs are checked in definition order
#[test]
fn graph_order() {
    run_test("connection_direct/graph_order")
}
