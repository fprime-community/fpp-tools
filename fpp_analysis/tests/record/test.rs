use crate::run_test;

#[test]
fn id_not_numeric() {
    run_test("record/id_not_numeric")
}

#[test]
fn ok() {
    run_test("record/ok")
}

#[test]
fn missing_container() {
    run_test("record/missing_container")
}

#[test]
fn missing_product_recv_port() {
    run_test("record/missing_product_recv_port")
}

#[test]
fn missing_ports() {
    run_test("record/missing_ports")
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
fn duplicate_id_implicit() {
    run_test("record/duplicate_id_implicit")
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
fn duplicate_id_explicit() {
    run_test("record/duplicate_id_explicit")
}

#[test]
fn missing_container_order() {
    run_test("record/missing_container_order")
}
