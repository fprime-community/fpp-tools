use crate::run_test;

#[test]
fn undef_instance() {
    run_test("connection_direct/undef_instance")
}

#[test]
fn typed_to_serial_with_return() {
    run_test("connection_direct/typed_to_serial_with_return")
}

#[test]
fn ok() {
    run_test("connection_direct/ok")
}

#[test]
fn invalid_port_instance() {
    run_test("connection_direct/invalid_port_instance")
}

#[test]
fn serial_to_typed_with_return() {
    run_test("connection_direct/serial_to_typed_with_return")
}

#[test]
fn invalid_directions() {
    run_test("connection_direct/invalid_directions")
}

#[test]
fn invalid_port_number() {
    run_test("connection_direct/invalid_port_number")
}

#[test]
fn mismatched_port_types() {
    run_test("connection_direct/mismatched_port_types")
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
fn instance_not_in_topology() {
    run_test("connection_direct/instance_not_in_topology")
}

#[test]
fn internal_port() {
    run_test("connection_direct/internal_port")
}
