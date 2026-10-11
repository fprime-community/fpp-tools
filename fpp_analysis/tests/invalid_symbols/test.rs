use crate::run_test;

#[test]
fn constant_as_type() {
    run_test("invalid_symbols/constant_as_type")
}

#[test]
fn constant_integer_as_qualifier() {
    run_test("invalid_symbols/constant_integer_as_qualifier")
}

#[test]
fn module_as_component() {
    run_test("invalid_symbols/module_as_component")
}

#[test]
fn module_as_component_instance() {
    run_test("invalid_symbols/module_as_component_instance")
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
fn module_as_state_machine() {
    run_test("invalid_symbols/module_as_state_machine")
}

#[test]
fn module_as_topology() {
    run_test("invalid_symbols/module_as_topology")
}

#[test]
fn module_as_type() {
    run_test("invalid_symbols/module_as_type")
}

#[test]
fn module_hides_constant() {
    run_test("invalid_symbols/module_hides_constant")
}

#[test]
fn topology_as_qualifier() {
    run_test("invalid_symbols/topology_as_qualifier")
}

#[test]
fn type_as_constant() {
    run_test("invalid_symbols/type_as_constant")
}

// Rust-only tests (no Scala counterpart)

/// A qualified name that names an enum type is not a constant
#[test]
fn enum_type_as_constant() {
    run_test("invalid_symbols/enum_type_as_constant")
}

/// A qualified name that names a state machine is not a constant
#[test]
fn state_machine_as_constant() {
    run_test("invalid_symbols/state_machine_as_constant")
}

/// A qualified enum type name is not a constant
#[test]
fn enum_as_qualified_constant() {
    run_test("invalid_symbols/enum_as_qualified_constant")
}

/// A qualified state machine name is not a constant
#[test]
fn state_machine_as_qualified_constant() {
    run_test("invalid_symbols/state_machine_as_qualified_constant")
}

/// A qualified module name is not a constant
#[test]
fn module_as_qualified_constant() {
    run_test("invalid_symbols/module_as_qualified_constant")
}

/// An enum constant has no members to select
#[test]
fn enum_constant_as_qualifier() {
    run_test("invalid_symbols/enum_constant_as_qualifier")
}

/// A module is not a component instance or topology in a connection
#[test]
fn module_as_connection_endpoint() {
    run_test("invalid_symbols/module_as_connection_endpoint")
}

/// A module is not an interface in an implements clause
#[test]
fn module_as_implemented_interface() {
    run_test("invalid_symbols/module_as_implemented_interface")
}

/// A module is not a component instance or topology as a template argument
#[test]
fn module_as_interface_instance_arg() {
    run_test("invalid_symbols/module_as_interface_instance_arg")
}

/// A module is not an interface as the type of an instance parameter
#[test]
fn module_as_interface_param_type() {
    run_test("invalid_symbols/module_as_interface_param_type")
}

/// A module is not a component instance or topology in a topology port
#[test]
fn module_as_topology_port() {
    run_test("invalid_symbols/module_as_topology_port")
}
