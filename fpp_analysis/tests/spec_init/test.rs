use crate::run_test;

#[test]
fn duplicate_phase() {
    run_test("spec_init/duplicate_phase")
}

#[test]
fn ok() {
    run_test("spec_init/ok")
}

#[test]
fn undef_phase() {
    run_test("spec_init/undef_phase")
}

// Rust-only tests (no Scala counterpart)

#[test]
fn phase_out_of_range() {
    run_test("spec_init/phase_out_of_range")
}
