use crate::run_test;

#[test]
fn abs_type_dictionary_error() {
    run_test("spec_loc/abs_type_dictionary_error")
}

#[test]
fn abs_type_ok() {
    run_test("spec_loc/abs_type_ok")
}

#[test]
fn abs_type_path_error() {
    run_test("spec_loc/abs_type_path_error")
}

#[test]
fn alias_type_dictionary_error() {
    run_test("spec_loc/alias_type_dictionary_error")
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
fn array_dictionary_error() {
    run_test("spec_loc/array_dictionary_error")
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
fn component_instance_ok() {
    run_test("spec_loc/component_instance_ok")
}

#[test]
fn component_instance_path_error() {
    run_test("spec_loc/component_instance_path_error")
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
fn constant_dictionary_error() {
    run_test("spec_loc/constant_dictionary_error")
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
fn enum_dictionary_error() {
    run_test("spec_loc/enum_dictionary_error")
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
fn shadowed_error() {
    run_test("spec_loc/shadowed_error")
}

#[test]
fn shadowed_ok() {
    run_test("spec_loc/shadowed_ok")
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
fn struct_dictionary_error() {
    run_test("spec_loc/struct_dictionary_error")
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
fn system_ok() {
    run_test("spec_loc/system_ok")
}

#[test]
fn system_path_error() {
    run_test("spec_loc/system_path_error")
}

#[test]
fn template_body_expanded_path_error() {
    run_test("spec_loc/template_body_expanded_path_error")
}

#[test]
fn template_body_inconsistent_path_error() {
    run_test("spec_loc/template_body_inconsistent_path_error")
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
fn template_ok() {
    run_test("spec_loc/template_ok")
}

#[test]
fn template_path_error() {
    run_test("spec_loc/template_path_error")
}

#[test]
fn topology_ok() {
    run_test("spec_loc/topology_ok")
}

#[test]
fn topology_path_error() {
    run_test("spec_loc/topology_path_error")
}

// Rust-only tests (no Scala counterpart)

/// Location specifiers for definitions nested two or more modules deep
#[test]
fn nested_ok() {
    run_test("spec_loc/nested_ok")
}

/// An incorrect location path for a definition nested two modules deep
#[test]
fn nested_path_error() {
    run_test("spec_loc/nested_path_error")
}

/// A location specifier for a four-part name that no definition has
#[test]
fn four_part_name_ok() {
    run_test("spec_loc/four_part_name_ok")
}

/// An incorrect location path for a four-part name
#[test]
fn four_part_name_path_error() {
    run_test("spec_loc/four_part_name_path_error")
}

/// Location paths are normalized before they are compared
#[test]
fn relative_path_ok() {
    run_test("spec_loc/relative_path_ok")
}

/// A normalized location path that names another file is incorrect
#[test]
fn relative_path_error() {
    run_test("spec_loc/relative_path_error")
}

/// A location path in an included file is relative to the included file
#[test]
fn relative_path_include_ok() {
    run_test("spec_loc/relative_path_include_ok")
}

/// Two specifiers that spell the same file differently are consistent
#[test]
fn equivalent_paths_ok() {
    run_test("spec_loc/equivalent_paths_ok")
}
