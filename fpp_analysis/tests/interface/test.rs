use crate::run_test;

#[test]
fn async_port_in_passive() {
    run_test("interface/async_port_in_passive")
}

#[test]
fn conflict_name() {
    run_test("interface/conflict_name")
}

#[test]
fn cycles() {
    run_test("interface/cycles")
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
fn empty_ok() {
    run_test("interface/empty_ok")
}

#[test]
fn ok() {
    run_test("interface/ok")
}

// Rust-only tests (no Scala counterpart)

/// Importing the same port twice through two interfaces
#[test]
fn diamond_import() {
    run_test("interface/diamond_import")
}

/// The first of two conflicting imports is reported
#[test]
fn import_conflict_order() {
    run_test("interface/import_conflict_order")
}

/// A cycle of interface imports
#[test]
fn import_cycle() {
    run_test("interface/import_cycle")
}

/// An import of an undefined interface
#[test]
fn undefined_import() {
    run_test("interface/undefined_import")
}

/// The error in an imported interface is reported at its definition
#[test]
fn import_of_broken_interface() {
    run_test("interface/import_of_broken_interface")
}

/// The first duplicate port of an import is reported, in the order of the import
#[test]
fn import_duplicate_port_order() {
    run_test("interface/import_duplicate_port_order")
}

/// A duplicate port of an imported interface is reported in the order of its port map
#[test]
fn import_duplicate_port_import_order() {
    run_test("interface/import_duplicate_port_import_order")
}

/// The first duplicate special port of an import is reported
#[test]
fn import_duplicate_special_port_order() {
    run_test("interface/import_duplicate_special_port_order")
}

/// An interface keeps the ports it merged before a failed import
#[test]
fn failed_import_keeps_merged_ports() {
    run_test("interface/failed_import_keeps_merged_ports")
}

/// A module named in an interface import is not an interface
#[test]
fn import_module() {
    run_test("interface/import_module")
}
