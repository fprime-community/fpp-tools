use crate::run_test;

#[test]
fn negative_throttle() {
    run_test("event/negative_throttle")
}

#[test]
fn bad_throttle_interval() {
    run_test("event/bad_throttle_interval")
}

#[test]
fn ref_params() {
    run_test("event/ref_params")
}

#[test]
fn bad_throttle() {
    run_test("event/bad_throttle")
}

#[test]
fn format_alias_not_numeric() {
    run_test("event/format_alias_not_numeric")
}

#[test]
fn bad_throttle_seconds() {
    run_test("event/bad_throttle_seconds")
}

#[test]
fn ok() {
    run_test("event/ok")
}

#[test]
fn negative_id() {
    run_test("event/negative_id")
}

#[test]
fn bad_id() {
    run_test("event/bad_id")
}

#[test]
fn missing_ports() {
    run_test("event/missing_ports")
}

#[test]
fn duplicate_name() {
    run_test("event/duplicate_name")
}

#[test]
fn format_not_numeric() {
    run_test("event/format_not_numeric")
}

#[test]
fn bad_throttle_interval_seconds() {
    run_test("event/bad_throttle_interval_seconds")
}

#[test]
fn format_missing_repl() {
    run_test("event/format_missing_repl")
}

#[test]
fn throttle_too_large() {
    run_test("event/throttle_too_large")
}

#[test]
fn duplicate_id_implicit() {
    run_test("event/duplicate_id_implicit")
}

#[test]
fn bad_throttle_interval_useconds() {
    run_test("event/bad_throttle_interval_useconds")
}

#[test]
fn zero_throttle_count() {
    run_test("event/zero_throttle_count")
}

#[test]
fn format_too_many_repls() {
    run_test("event/format_too_many_repls")
}

#[test]
fn bad_throttle_interval_extra_member() {
    run_test("event/bad_throttle_interval_extra_member")
}

#[test]
fn not_displayable() {
    run_test("event/not_displayable")
}

#[test]
fn duplicate_id_explicit() {
    run_test("event/duplicate_id_explicit")
}

#[test]
fn duplicate_name_descending_ids() {
    run_test("event/duplicate_name_descending_ids")
}
