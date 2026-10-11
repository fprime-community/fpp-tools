use crate::run_test;

#[test]
fn type_as_constant() {
    run_test("invalid_symbols/type_as_constant")
}

#[test]
fn module_as_type() {
    run_test("invalid_symbols/module_as_type")
}

#[test]
fn module_as_constant() {
    run_test("invalid_symbols/module_as_constant")
}

#[test]
fn module_as_port() {
    run_test("invalid_symbols/module_as_port")
}

#[test]
fn constant_integer_as_qualifier() {
    run_test("invalid_symbols/constant_integer_as_qualifier")
}

#[test]
fn module_hides_constant() {
    run_test("invalid_symbols/module_hides_constant")
}

#[test]
fn constant_as_type() {
    run_test("invalid_symbols/constant_as_type")
}

#[test]
fn state_machine_as_qualifier() {
    run_test("invalid_symbols/state_machine_as_qualifier")
}

#[test]
fn module_as_topology() {
    run_test("invalid_symbols/module_as_topology")
}

#[test]
fn topology_as_qualifier() {
    run_test("invalid_symbols/topology_as_qualifier")
}

#[test]
fn module_as_component() {
    run_test("invalid_symbols/module_as_component")
}

#[test]
fn module_as_state_machine() {
    run_test("invalid_symbols/module_as_state_machine")
}

#[test]
fn module_as_component_instance() {
    run_test("invalid_symbols/module_as_component_instance")
}

#[test]
fn module_as_connection_endpoint() {
    run_test("invalid_symbols/module_as_connection_endpoint")
}

#[test]
fn module_as_implemented_interface() {
    run_test("invalid_symbols/module_as_implemented_interface")
}

#[test]
fn module_as_interface_instance_arg() {
    run_test("invalid_symbols/module_as_interface_instance_arg")
}

#[test]
fn module_as_interface_param_type() {
    run_test("invalid_symbols/module_as_interface_param_type")
}

#[test]
fn module_as_topology_port() {
    run_test("invalid_symbols/module_as_topology_port")
}
