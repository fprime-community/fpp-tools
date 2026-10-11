use crate::run_test;

#[test]
fn async_passive() {
    run_test("command/async_passive")
}

#[test]
fn bad_opcode() {
    run_test("command/bad_opcode")
}

#[test]
fn bad_priority() {
    run_test("command/bad_priority")
}

#[test]
fn duplicate_name() {
    run_test("command/duplicate_name")
}

#[test]
fn duplicate_name_param_save() {
    run_test("command/duplicate_name_param_save")
}

#[test]
fn duplicate_name_param_set() {
    run_test("command/duplicate_name_param_set")
}

#[test]
fn duplicate_opcode_explicit() {
    run_test("command/duplicate_opcode_explicit")
}

#[test]
fn duplicate_opcode_implicit() {
    run_test("command/duplicate_opcode_implicit")
}

#[test]
fn duplicate_param() {
    run_test("command/duplicate_param")
}

#[test]
fn missing_ports() {
    run_test("command/missing_ports")
}

#[test]
fn negative_opcode() {
    run_test("command/negative_opcode")
}

#[test]
fn not_displayable() {
    run_test("command/not_displayable")
}

#[test]
fn ok() {
    run_test("command/ok")
}

#[test]
fn ref_params() {
    run_test("command/ref_params")
}

#[test]
fn sync_priority() {
    run_test("command/sync_priority")
}

#[test]
fn sync_queue_full() {
    run_test("command/sync_queue_full")
}

// Rust-only tests (no Scala counterpart)

/// A guarded command may not have queue full behavior
#[test]
fn guarded_queue_full() {
    run_test("command/guarded_queue_full")
}

/// Duplicate command names are reported in model order, whatever the opcodes
#[test]
fn duplicate_name_descending_opcodes() {
    run_test("command/duplicate_name_descending_opcodes")
}

/// A default opcode past the 128-bit integer range is an error
#[test]
fn opcode_overflow() {
    run_test("command/opcode_overflow")
}
