use crate::run_test;

#[test]
fn empty_ok() {
    run_test("interface/empty_ok")
}

#[test]
fn ok() {
    run_test("interface/ok")
}

#[test]
fn duplicate_import() {
    run_test("interface/duplicate_import")
}

#[test]
fn duplicate_name() {
    run_test("interface/duplicate_name")
}

#[test]
fn cycles() {
    run_test("interface/cycles")
}

#[test]
fn conflict_name() {
    run_test("interface/conflict_name")
}

#[test]
fn async_port_in_passive() {
    run_test("interface/async_port_in_passive")
}

#[test]
fn diamond_import() {
    run_test("interface/diamond_import")
}

#[test]
fn import_conflict_order() {
    run_test("interface/import_conflict_order")
}

#[test]
fn import_cycle() {
    run_test("interface/import_cycle")
}

#[test]
fn undefined_import() {
    run_test("interface/undefined_import")
}

#[test]
fn import_of_broken_interface() {
    run_test("interface/import_of_broken_interface")
}

#[test]
fn import_duplicate_port_order() {
    run_test("interface/import_duplicate_port_order")
}

#[test]
fn import_duplicate_port_import_order() {
    run_test("interface/import_duplicate_port_import_order")
}

#[test]
fn import_duplicate_special_port_order() {
    run_test("interface/import_duplicate_special_port_order")
}

#[test]
fn failed_import_keeps_merged_ports() {
    run_test("interface/failed_import_keeps_merged_ports")
}
