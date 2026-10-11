use crate::run_test;

#[test]
fn duplicate_connection_at_matched_port() {
    run_test("port_numbering/duplicate_connection_at_matched_port")
}

#[test]
fn duplicate_matched_connection() {
    run_test("port_numbering/duplicate_matched_connection")
}

#[test]
fn duplicate_output_connection() {
    run_test("port_numbering/duplicate_output_connection")
}

#[test]
fn implicit_duplicate_connection_at_matched_input_port() {
    run_test("port_numbering/implicit_duplicate_connection_at_matched_input_port")
}

#[test]
fn implicit_duplicate_connection_at_matched_output_port() {
    run_test("port_numbering/implicit_duplicate_connection_at_matched_output_port")
}

#[test]
fn mismatched_port_numbers() {
    run_test("port_numbering/mismatched_port_numbers")
}

#[test]
fn missing_connection() {
    run_test("port_numbering/missing_connection")
}

#[test]
fn negative_port_number() {
    run_test("port_numbering/negative_port_number")
}

#[test]
fn no_port_available_for_matched_numbering() {
    run_test("port_numbering/no_port_available_for_matched_numbering")
}

#[test]
fn ok() {
    run_test("port_numbering/ok")
}

#[test]
fn template_implicit_duplicate_at_matched_port() {
    run_test("port_numbering/template_implicit_duplicate_at_matched_port")
}

#[test]
fn template_ok() {
    run_test("port_numbering/template_ok")
}

#[test]
fn template_too_many_output_ports() {
    run_test("port_numbering/template_too_many_output_ports")
}

#[test]
fn too_many_output_ports() {
    run_test("port_numbering/too_many_output_ports")
}

// Rust-only tests (no Scala counterpart)

/// Port numbering after a redefined instance name
#[test]
fn duplicate_instance_name() {
    run_test("port_numbering/duplicate_instance_name")
}

/// Matched numbering through a topology port alias
#[test]
fn matched_through_alias() {
    run_test("port_numbering/matched_through_alias")
}

/// Too many output ports connected by two expansions of one template
#[test]
fn template_two_expansions_too_many_output_ports() {
    run_test("port_numbering/template_two_expansions_too_many_output_ports")
}

/// A duplicate output connection made by two expansions of one template
#[test]
fn template_two_expansions_duplicate_output_connection() {
    run_test("port_numbering/template_two_expansions_duplicate_output_connection")
}

/// A port number outside the 32-bit range is an error
#[test]
fn port_number_out_of_range() {
    run_test("port_numbering/port_number_out_of_range")
}

/// A connection port number must have a numeric type
#[test]
fn port_number_not_numeric() {
    run_test("port_numbering/port_number_not_numeric")
}

/// The port numbers of a connection are checked after the component definitions
#[test]
fn port_number_not_numeric_after_component_error() {
    run_test("port_numbering/port_number_not_numeric_after_component_error")
}
