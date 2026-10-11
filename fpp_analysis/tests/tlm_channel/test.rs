use crate::run_test;

#[test]
fn bad_id() {
    run_test("tlm_channel/bad_id")
}

#[test]
fn bad_limit_type() {
    run_test("tlm_channel/bad_limit_type")
}

#[test]
fn duplicate_id_explicit() {
    run_test("tlm_channel/duplicate_id_explicit")
}

#[test]
fn duplicate_id_implicit() {
    run_test("tlm_channel/duplicate_id_implicit")
}

#[test]
fn duplicate_limit_high() {
    run_test("tlm_channel/duplicate_limit_high")
}

#[test]
fn duplicate_limit_low() {
    run_test("tlm_channel/duplicate_limit_low")
}

#[test]
fn duplicate_name() {
    run_test("tlm_channel/duplicate_name")
}

#[test]
fn format_alias_not_numeric() {
    run_test("tlm_channel/format_alias_not_numeric")
}

#[test]
fn format_missing_repl() {
    run_test("tlm_channel/format_missing_repl")
}

#[test]
fn format_not_numeric() {
    run_test("tlm_channel/format_not_numeric")
}

#[test]
fn format_too_many_repls() {
    run_test("tlm_channel/format_too_many_repls")
}

#[test]
fn limit_not_numeric() {
    run_test("tlm_channel/limit_not_numeric")
}

#[test]
fn missing_ports() {
    run_test("tlm_channel/missing_ports")
}

#[test]
fn negative_id() {
    run_test("tlm_channel/negative_id")
}

#[test]
fn not_displayable() {
    run_test("tlm_channel/not_displayable")
}

#[test]
fn ok() {
    run_test("tlm_channel/ok")
}
