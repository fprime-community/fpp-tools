use crate::run_test;

#[test]
fn duplicate_id_explicit() {
    run_test("record/duplicate_id_explicit")
}

#[test]
fn duplicate_id_implicit() {
    run_test("record/duplicate_id_implicit")
}

#[test]
fn duplicate_name() {
    run_test("record/duplicate_name")
}

#[test]
fn id_negative() {
    run_test("record/id_negative")
}

#[test]
fn id_not_numeric() {
    run_test("record/id_not_numeric")
}

#[test]
fn missing_container() {
    run_test("record/missing_container")
}

#[test]
fn missing_ports() {
    run_test("record/missing_ports")
}

#[test]
fn missing_product_recv_port() {
    run_test("record/missing_product_recv_port")
}

#[test]
fn missing_product_send_port() {
    run_test("record/missing_product_send_port")
}

#[test]
fn not_displayable() {
    run_test("record/not_displayable")
}

#[test]
fn ok() {
    run_test("record/ok")
}

// Rust-only tests (no Scala counterpart)

/// A component with records and no containers is reported once
#[test]
fn missing_container_order() {
    run_test("record/missing_container_order")
}
