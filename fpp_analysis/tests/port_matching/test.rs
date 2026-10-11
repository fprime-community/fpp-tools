use crate::run_test;

#[test]
fn p2_not_port_instance() {
    run_test("port_matching/p2_not_port_instance")
}

#[test]
fn ok() {
    run_test("port_matching/ok")
}

#[test]
fn p1_not_valid() {
    run_test("port_matching/p1_not_valid")
}

#[test]
fn p1_not_port_instance() {
    run_test("port_matching/p1_not_port_instance")
}

#[test]
fn mismatched_sizes() {
    run_test("port_matching/mismatched_sizes")
}

#[test]
fn repeated_name() {
    run_test("port_matching/repeated_name")
}

#[test]
fn p2_not_valid() {
    run_test("port_matching/p2_not_valid")
}

#[test]
fn missing_connection() {
    run_test("port_matching/missing_connection")
}

#[test]
fn template_matched_remote_arg() {
    run_test("port_matching/template_matched_remote_arg")
}

#[test]
fn template_mismatched_port_numbers() {
    run_test("port_matching/template_mismatched_port_numbers")
}

#[test]
fn template_missing_connection() {
    run_test("port_matching/template_missing_connection")
}

#[test]
fn template_ok() {
    run_test("port_matching/template_ok")
}

#[test]
fn template_unmatched() {
    run_test("port_matching/template_unmatched")
}

#[test]
fn template_unmatched_remote_arg() {
    run_test("port_matching/template_unmatched_remote_arg")
}

#[test]
fn missing_connection_order() {
    run_test("port_matching/missing_connection_order")
}
