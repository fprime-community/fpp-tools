use crate::run_test;

#[test]
fn alias_rep_type_ok() {
    run_test("enum/alias_rep_type_ok")
}

#[test]
fn bad_alias_rep_type() {
    run_test("enum/bad_alias_rep_type")
}

#[test]
fn bad_constant() {
    run_test("enum/bad_constant")
}

#[test]
fn bad_default() {
    run_test("enum/bad_default")
}

#[test]
fn bad_rep_type() {
    run_test("enum/bad_rep_type")
}

#[test]
fn default_ok() {
    run_test("enum/default_ok")
}

#[test]
fn default_shadowed_error() {
    run_test("enum/default_shadowed_error")
}

#[test]
fn default_shadowed_ok() {
    run_test("enum/default_shadowed_ok")
}

#[test]
fn dictionary_ok() {
    run_test("enum/dictionary_ok")
}

#[test]
fn duplicate_value() {
    run_test("enum/duplicate_value")
}

#[test]
fn explicit() {
    run_test("enum/explicit")
}

#[test]
fn implied() {
    run_test("enum/implied")
}

#[test]
fn invalid_constants() {
    run_test("enum/invalid_constants")
}

#[test]
fn invalid_symbol() {
    run_test("enum/invalid_symbol")
}

#[test]
fn missing_constant() {
    run_test("enum/missing_constant")
}

#[test]
fn undef_constant_1() {
    run_test("enum/undef_constant_1")
}

#[test]
fn undef_constant_2() {
    run_test("enum/undef_constant_2")
}

// Rust-only tests (no Scala counterpart)

/// One error at the enum, however many constants mix explicit and implied values
#[test]
fn invalid_constants_mixed() {
    run_test("enum/invalid_constants_mixed")
}

/// The mixed-constants error is at the enum definition
#[test]
fn invalid_constants_multiline() {
    run_test("enum/invalid_constants_multiline")
}

/// Mixed constants are reported before duplicate values
#[test]
fn invalid_constants_duplicate_value() {
    run_test("enum/invalid_constants_duplicate_value")
}

/// The size of a string representation type is resolved outside the enum
#[test]
fn rep_type_string_size_scope() {
    run_test("enum/rep_type_string_size_scope")
}
