use crate::run_test;

#[test]
fn duplicate_output_connection() {
    run_test("port_numbering/duplicate_output_connection")
}

#[test]
fn no_port_available_for_matched_numbering() {
    run_test("port_numbering/no_port_available_for_matched_numbering")
}

#[test]
fn implicit_duplicate_connection_at_matched_output_port() {
    run_test("port_numbering/implicit_duplicate_connection_at_matched_output_port")
}

#[test]
fn ok() {
    run_test("port_numbering/ok")
}

#[test]
fn mismatched_port_numbers() {
    run_test("port_numbering/mismatched_port_numbers")
}

#[test]
fn too_many_output_ports() {
    run_test("port_numbering/too_many_output_ports")
}

#[test]
fn duplicate_matched_connection() {
    run_test("port_numbering/duplicate_matched_connection")
}

#[test]
fn negative_port_number() {
    run_test("port_numbering/negative_port_number")
}

#[test]
fn implicit_duplicate_connection_at_matched_input_port() {
    run_test("port_numbering/implicit_duplicate_connection_at_matched_input_port")
}

#[test]
fn duplicate_connection_at_matched_port() {
    run_test("port_numbering/duplicate_connection_at_matched_port")
}

#[test]
fn missing_connection() {
    run_test("port_numbering/missing_connection")
}

#[test]
fn duplicate_instance_name() {
    run_test("port_numbering/duplicate_instance_name")
}

#[test]
fn matched_through_alias() {
    run_test("port_numbering/matched_through_alias")
}
