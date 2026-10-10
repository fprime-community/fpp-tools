use crate::run_test;

#[test]
fn abs_type_ok() {
    run_test("spec_loc/abs_type_ok")
}

// Deeply-nested (2+ module) located definitions: confirm the path-lookup key
// (`get_qualified_name`) matches the located-symbol key for definitions nested
// more than one module deep (fidelity open question, agent 6).
#[test]
fn nested_ok() {
    run_test("spec_loc/nested_ok")
}

#[test]
fn nested_path_error() {
    run_test("spec_loc/nested_path_error")
}

#[test]
fn abs_type_path_error() {
    run_test("spec_loc/abs_type_path_error")
}

#[test]
fn abs_type_dictionary_error() {
    run_test("spec_loc/abs_type_dictionary_error")
}

#[test]
fn alias_type_ok() {
    run_test("spec_loc/alias_type_ok")
}

#[test]
fn alias_type_path_error() {
    run_test("spec_loc/alias_type_path_error")
}

#[test]
fn alias_type_dictionary_error() {
    run_test("spec_loc/alias_type_dictionary_error")
}

#[test]
fn array_ok() {
    run_test("spec_loc/array_ok")
}

#[test]
fn array_path_error() {
    run_test("spec_loc/array_path_error")
}

#[test]
fn array_dictionary_error() {
    run_test("spec_loc/array_dictionary_error")
}

#[test]
fn component_ok() {
    run_test("spec_loc/component_ok")
}

#[test]
fn component_path_error() {
    run_test("spec_loc/component_path_error")
}

#[test]
fn component_instance_ok() {
    run_test("spec_loc/component_instance_ok")
}

#[test]
fn component_instance_path_error() {
    run_test("spec_loc/component_instance_path_error")
}

#[test]
fn constant_ok() {
    run_test("spec_loc/constant_ok")
}

#[test]
fn constant_path_error() {
    run_test("spec_loc/constant_path_error")
}

#[test]
fn constant_dictionary_error() {
    run_test("spec_loc/constant_dictionary_error")
}

#[test]
fn enum_ok() {
    run_test("spec_loc/enum_ok")
}

#[test]
fn enum_path_error() {
    run_test("spec_loc/enum_path_error")
}

#[test]
fn enum_dictionary_error() {
    run_test("spec_loc/enum_dictionary_error")
}

#[test]
fn include_ok() {
    run_test("spec_loc/include_ok")
}

#[test]
fn interface_ok() {
    run_test("spec_loc/interface_ok")
}

#[test]
fn interface_path_error() {
    run_test("spec_loc/interface_path_error")
}

#[test]
fn port_ok() {
    run_test("spec_loc/port_ok")
}

#[test]
fn port_path_error() {
    run_test("spec_loc/port_path_error")
}

#[test]
fn state_machine_ok() {
    run_test("spec_loc/state_machine_ok")
}

#[test]
fn state_machine_path_error() {
    run_test("spec_loc/state_machine_path_error")
}

#[test]
fn struct_ok() {
    run_test("spec_loc/struct_ok")
}

#[test]
fn struct_path_error() {
    run_test("spec_loc/struct_path_error")
}

#[test]
fn struct_dictionary_error() {
    run_test("spec_loc/struct_dictionary_error")
}

#[test]
fn system_ok() {
    run_test("spec_loc/system_ok")
}

#[test]
fn system_path_error() {
    run_test("spec_loc/system_path_error")
}

#[test]
fn topology_ok() {
    run_test("spec_loc/topology_ok")
}

#[test]
fn topology_path_error() {
    run_test("spec_loc/topology_path_error")
}

#[test]
fn template_ok() {
    run_test("spec_loc/template_ok")
}

#[test]
fn template_path_error() {
    run_test("spec_loc/template_path_error")
}

#[test]
fn template_body_ok() {
    run_test("spec_loc/template_body_ok")
}

#[test]
fn template_body_path_error() {
    run_test("spec_loc/template_body_path_error")
}

#[test]
fn template_body_inconsistent_path_error() {
    run_test("spec_loc/template_body_inconsistent_path_error")
}

#[test]
fn template_body_expanded_path_error() {
    run_test("spec_loc/template_body_expanded_path_error")
}
