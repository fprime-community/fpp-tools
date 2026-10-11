use crate::{run_test, run_test_with};

#[test]
fn basic() {
    run_test("top_import/basic")
}

#[test]
fn deployment_topology() {
    run_test("top_import/deployment_topology")
}

#[test]
fn duplicate_topology() {
    run_test("top_import/duplicate_topology")
}

#[test]
fn instance_public() {
    run_test("top_import/instance_public")
}

#[test]
fn shadowed_error() {
    run_test("top_import/shadowed_error")
}

#[test]
fn shadowed_ok() {
    run_test("top_import/shadowed_ok")
}

#[test]
fn undef_topology() {
    run_test("top_import/undef_topology")
}

// Rust-only tests (no Scala counterpart)

/// A topology that an earlier topology imports is resolved first
#[test]
fn import_order_instances() {
    run_test("top_import/import_order_instances")
}

/// Imported topologies are resolved in import order
#[test]
fn import_order_connections() {
    run_test("top_import/import_order_connections")
}

/// A topology in an included file is resolved at the include
#[test]
fn include_order() {
    run_test("top_import/include_order")
}

/// Topologies are resolved in the order of the input files
#[test]
fn file_order() {
    run_test_with(
        &["top_import/file_order_common", "top_import/file_order_b"],
        "top_import/file_order_a",
    )
}
