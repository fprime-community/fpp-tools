use crate::run_test;

#[test]
fn alias_type_ok() {
    run_test("types/alias_type_ok")
}

#[test]
fn uses_ok() {
    run_test("types/uses_ok")
}

#[test]
fn string_size_negative() {
    run_test("types/string_size_negative")
}

#[test]
fn string_size_too_large() {
    run_test("types/string_size_too_large")
}

#[test]
fn string_size_not_numeric() {
    run_test("types/string_size_not_numeric")
}

#[test]
fn alias_dictionary_not_displayable() {
    run_test("types/alias_dictionary_not_displayable")
}

#[test]
fn alias_dictionary_ok() {
    run_test("types/alias_dictionary_ok")
}

#[test]
fn string_length_shadowed() {
    run_test("types/string_length_shadowed")
}

#[test]
fn string_missing_default_size_constant() {
    run_test("types/string_missing_default_size_constant")
}

#[test]
fn string_implied_constant_unop_order() {
    run_test("types/string_implied_constant_unop_order")
}

#[test]
fn string_missing_fw_size_store_type() {
    run_test("types/string_missing_fw_size_store_type")
}

#[test]
fn string_size_type_shadowed() {
    run_test("types/string_size_type_shadowed")
}

#[test]
fn string_size_zero_ok() {
    run_test("types/string_size_zero_ok")
}

#[test]
fn string_implied_type_not_type() {
    run_test("types/string_implied_type_not_type")
}

#[test]
fn string_implied_constant_not_constant() {
    run_test("types/string_implied_constant_not_constant")
}

#[test]
fn string_implied_constant_module() {
    run_test("types/string_implied_constant_module")
}

#[test]
fn string_implied_constant_order() {
    run_test("types/string_implied_constant_order")
}

#[test]
fn string_implied_type_order() {
    run_test("types/string_implied_type_order")
}

#[test]
fn alias_string_size_not_numeric() {
    run_test("types/alias_string_size_not_numeric")
}
