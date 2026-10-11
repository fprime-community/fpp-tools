use crate::run_test;

#[test]
fn alias_dictionary_not_displayable() {
    run_test("types/alias_dictionary_not_displayable")
}

#[test]
fn alias_dictionary_ok() {
    run_test("types/alias_dictionary_ok")
}

#[test]
fn alias_type_ok() {
    run_test("types/alias_type_ok")
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
fn string_missing_fw_size_store_type() {
    run_test("types/string_missing_fw_size_store_type")
}

#[test]
fn string_size_negative() {
    run_test("types/string_size_negative")
}

#[test]
fn string_size_not_numeric() {
    run_test("types/string_size_not_numeric")
}

#[test]
fn string_size_too_large() {
    run_test("types/string_size_too_large")
}

#[test]
fn string_size_zero_ok() {
    run_test("types/string_size_zero_ok")
}

#[test]
fn string_size_type_shadowed() {
    run_test("types/string_size_type_shadowed")
}

#[test]
fn uses_ok() {
    run_test("types/uses_ok")
}

// Rust-only tests (no Scala counterpart)

/// The size constant of a default-size string type is checked at the string type
#[test]
fn string_implied_constant_unop_order() {
    run_test("types/string_implied_constant_unop_order")
}

/// The implied FwSizeStoreType of a string type must be a type
#[test]
fn string_implied_type_not_type() {
    run_test("types/string_implied_type_not_type")
}

/// The implied size constant of a string type must be a constant
#[test]
fn string_implied_constant_not_constant() {
    run_test("types/string_implied_constant_not_constant")
}

/// The implied size constant of a string type may not be a module
#[test]
fn string_implied_constant_module() {
    run_test("types/string_implied_constant_module")
}

/// The implied size constant of a string type is checked at the string type
#[test]
fn string_implied_constant_order() {
    run_test("types/string_implied_constant_order")
}

/// The implied FwSizeStoreType of a string type is checked at the string type
#[test]
fn string_implied_type_order() {
    run_test("types/string_implied_type_order")
}

/// The size of a string type in an alias type definition must have a numeric type
#[test]
fn alias_string_size_not_numeric() {
    run_test("types/alias_string_size_not_numeric")
}

/// A state machine is not a type
#[test]
fn state_machine_not_a_type() {
    run_test("types/state_machine_not_a_type")
}
