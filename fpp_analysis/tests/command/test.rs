use crate::run_test;

#[test]
fn duplicate_name_param_save() {
    run_test("command/duplicate_name_param_save")
}

#[test]
fn ref_params() {
    run_test("command/ref_params")
}

#[test]
fn duplicate_name_param_set() {
    run_test("command/duplicate_name_param_set")
}

#[test]
fn negative_opcode() {
    run_test("command/negative_opcode")
}

#[test]
fn sync_priority() {
    run_test("command/sync_priority")
}

#[test]
fn duplicate_param() {
    run_test("command/duplicate_param")
}

#[test]
fn bad_priority() {
    run_test("command/bad_priority")
}

#[test]
fn ok() {
    run_test("command/ok")
}

#[test]
fn bad_opcode() {
    run_test("command/bad_opcode")
}

#[test]
fn sync_queue_full() {
    run_test("command/sync_queue_full")
}

#[test]
fn guarded_queue_full() {
    run_test("command/guarded_queue_full")
}

#[test]
fn duplicate_opcode_explicit() {
    run_test("command/duplicate_opcode_explicit")
}

#[test]
fn async_passive() {
    run_test("command/async_passive")
}

#[test]
fn missing_ports() {
    run_test("command/missing_ports")
}

#[test]
fn duplicate_name() {
    run_test("command/duplicate_name")
}

#[test]
fn duplicate_opcode_implicit() {
    run_test("command/duplicate_opcode_implicit")
}

#[test]
fn not_displayable() {
    run_test("command/not_displayable")
}
