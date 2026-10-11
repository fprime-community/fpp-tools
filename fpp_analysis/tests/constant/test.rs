use crate::run_test;

#[test]
fn array_common_type_conversion() {
    run_test("constant/array_common_type_conversion")
}

#[test]
fn array_index_negative() {
    run_test("constant/array_index_negative")
}

#[test]
fn array_index_out_of_bounds() {
    run_test("constant/array_index_out_of_bounds")
}

#[test]
fn dictionary_error() {
    run_test("constant/dictionary_error")
}

#[test]
fn dictionary_ok() {
    run_test("constant/dictionary_ok")
}

#[test]
fn invalid_array_index_type() {
    run_test("constant/invalid_array_index_type")
}

#[test]
fn invalid_array_type() {
    run_test("constant/invalid_array_type")
}

#[test]
fn shadowed_error() {
    run_test("constant/shadowed_error")
}

#[test]
fn shadowed_ok() {
    run_test("constant/shadowed_ok")
}

#[test]
fn undef_1() {
    run_test("constant/undef_1")
}

#[test]
fn undef_2() {
    run_test("constant/undef_2")
}

#[test]
fn undef_3() {
    run_test("constant/undef_3")
}

#[test]
fn uses_ok() {
    run_test("constant/uses_ok")
}

// Rust-only tests (no Scala counterpart)

/// An enum constant array index is converted to Integer
#[test]
fn array_index_enum_ok() {
    run_test("constant/array_index_enum_ok")
}

/// A floating-point array index is converted to Integer
#[test]
fn array_index_float_ok() {
    run_test("constant/array_index_float_ok")
}

/// Scalars and template arguments promoted into nested array constants
#[test]
fn template_scalar_array_promote_ok() {
    run_test("constant/template_scalar_array_promote_ok")
}

/// A subscript past the end of a promoted nested array constant
#[test]
fn template_scalar_array_promote_out_of_bounds() {
    run_test("constant/template_scalar_array_promote_out_of_bounds")
}

/// A dictionary constant may not have an alias type
#[test]
fn dictionary_alias_error() {
    run_test("constant/dictionary_alias_error")
}

/// An absolute constant name is looked up in the global scope only
#[test]
fn absolute_undef() {
    run_test("constant/absolute_undef")
}

/// A dictionary constant of array type in a state machine is accepted
#[test]
fn dictionary_state_machine_ok() {
    run_test("constant/dictionary_state_machine_ok")
}
