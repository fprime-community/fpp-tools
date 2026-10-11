use crate::run_test;

#[test]
fn duplicate_set_opcode_implicit() {
    run_test("param/duplicate_set_opcode_implicit")
}

#[test]
fn bad_set_opcode() {
    run_test("param/bad_set_opcode")
}

#[test]
fn bad_default() {
    run_test("param/bad_default")
}

#[test]
fn duplicate_set_opcode_explicit() {
    run_test("param/duplicate_set_opcode_explicit")
}

#[test]
fn ok() {
    run_test("param/ok")
}

#[test]
fn negative_id() {
    run_test("param/negative_id")
}

#[test]
fn negative_save_opcode() {
    run_test("param/negative_save_opcode")
}

#[test]
fn bad_save_opcode() {
    run_test("param/bad_save_opcode")
}

#[test]
fn duplicate_save_opcode_implicit() {
    run_test("param/duplicate_save_opcode_implicit")
}

#[test]
fn bad_id() {
    run_test("param/bad_id")
}

#[test]
fn missing_ports() {
    run_test("param/missing_ports")
}

#[test]
fn duplicate_name() {
    run_test("param/duplicate_name")
}

#[test]
fn duplicate_id_implicit() {
    run_test("param/duplicate_id_implicit")
}

#[test]
fn duplicate_save_opcode_explicit() {
    run_test("param/duplicate_save_opcode_explicit")
}

#[test]
fn negative_set_opcode() {
    run_test("param/negative_set_opcode")
}

#[test]
fn not_displayable() {
    run_test("param/not_displayable")
}

#[test]
fn duplicate_id_explicit() {
    run_test("param/duplicate_id_explicit")
}

#[test]
fn opcode_overflow() {
    run_test("param/opcode_overflow")
}

#[test]
fn default_opcode_after_32_bits() {
    run_test("param/default_opcode_after_32_bits")
}
