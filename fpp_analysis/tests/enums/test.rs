use crate::run_test;

#[test]
fn missing_constant() {
    run_test("enums/missing_constant")
}

#[test]
fn explicit() {
    run_test("enums/explicit")
}

#[test]
fn bad_default() {
    run_test("enums/bad_default")
}

#[test]
fn bad_alias_rep_type() {
    run_test("enums/bad_alias_rep_type")
}

#[test]
fn duplicate_value() {
    run_test("enums/duplicate_value")
}

#[test]
fn bad_constant() {
    run_test("enums/bad_constant")
}

#[test]
fn implied() {
    run_test("enums/implied")
}

#[test]
fn undef_constant_1() {
    run_test("enums/undef_constant_1")
}

#[test]
fn invalid_symbol() {
    run_test("enums/invalid_symbol")
}

#[test]
fn undef_constant_2() {
    run_test("enums/undef_constant_2")
}

#[test]
fn bad_rep_type() {
    run_test("enums/bad_rep_type")
}

#[test]
fn default_ok() {
    run_test("enums/default_ok")
}

#[test]
fn alias_rep_type_ok() {
    run_test("enums/alias_rep_type_ok")
}

#[test]
fn invalid_constants() {
    run_test("enums/invalid_constants")
}

#[test]
fn dictionary_ok() {
    run_test("enums/dictionary_ok")
}

#[test]
fn default_shadowed_error() {
    run_test("enums/default_shadowed_error")
}

#[test]
fn default_shadowed_ok() {
    run_test("enums/default_shadowed_ok")
}

#[test]
fn rep_type_string_size_scope() {
    run_test("enums/rep_type_string_size_scope")
}
