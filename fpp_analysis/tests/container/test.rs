use crate::run_test;

#[test]
fn duplicate_id_explicit() {
    run_test("container/duplicate_id_explicit")
}

#[test]
fn duplicate_id_implicit() {
    run_test("container/duplicate_id_implicit")
}

#[test]
fn duplicate_name() {
    run_test("container/duplicate_name")
}

#[test]
fn id_negative() {
    run_test("container/id_negative")
}

#[test]
fn id_not_numeric() {
    run_test("container/id_not_numeric")
}

#[test]
fn missing_ports() {
    run_test("container/missing_ports")
}

#[test]
fn missing_product_recv_port() {
    run_test("container/missing_product_recv_port")
}

#[test]
fn missing_product_send_port() {
    run_test("container/missing_product_send_port")
}

#[test]
fn missing_record() {
    run_test("container/missing_record")
}

#[test]
fn ok() {
    run_test("container/ok")
}

#[test]
fn priority_negative() {
    run_test("container/priority_negative")
}

#[test]
fn priority_not_numeric() {
    run_test("container/priority_not_numeric")
}

// Rust-only tests (no Scala counterpart)

/// A component with containers and no records is reported once
#[test]
fn missing_record_order() {
    run_test("container/missing_record_order")
}
