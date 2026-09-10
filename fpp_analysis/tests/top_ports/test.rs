use crate::run_test;

#[test]
fn implements_port_missing() {
    run_test("top_ports/implements_port_missing")
}

#[test]
fn implements_port_mismatch_1() {
    run_test("top_ports/implements_port_mismatch_1")
}

#[test]
fn nested() {
    run_test("top_ports/nested")
}

#[test]
fn unmatched_types() {
    run_test("top_ports/unmatched_types")
}

#[test]
fn implements_port_mismatch_2() {
    run_test("top_ports/implements_port_mismatch_2")
}

#[test]
fn implements() {
    run_test("top_ports/implements")
}

#[test]
fn basic() {
    run_test("top_ports/basic")
}

#[test]
fn out_to_out() {
    run_test("top_ports/out_to_out")
}

#[test]
fn internal_port() {
    run_test("top_ports/internal_port")
}

#[test]
fn interface_instance_not_member() {
    run_test("top_ports/interface_instance_not_member")
}

#[test]
fn top_to_top() {
    run_test("top_ports/top_to_top")
}

#[test]
fn unmatched_through_alias() {
    run_test("top_ports/unmatched_through_alias")
}

#[test]
fn implements_first_missing_port() {
    run_test("top_ports/implements_first_missing_port")
}

#[test]
fn implements_missing_port_import_order() {
    run_test("top_ports/implements_missing_port_import_order")
}

#[test]
fn implements_missing_port_beyond_four() {
    run_test("top_ports/implements_missing_port_beyond_four")
}
