use crate::run_test;

#[test]
fn default_error() {
    run_test("array/default_error")
}

#[test]
fn default_ok() {
    run_test("array/default_ok")
}

#[test]
fn default_shadowed_error() {
    run_test("array/default_shadowed_error")
}

#[test]
fn default_shadowed_ok() {
    run_test("array/default_shadowed_ok")
}

#[test]
fn dictionary_not_displayable() {
    run_test("array/dictionary_not_displayable")
}

#[test]
fn dictionary_ok() {
    run_test("array/dictionary_ok")
}

#[test]
fn enum_default_error() {
    run_test("array/enum_default_error")
}

#[test]
fn enum_default_ok() {
    run_test("array/enum_default_ok")
}

#[test]
fn enum_no_default_ok() {
    run_test("array/enum_no_default_ok")
}

#[test]
fn format_alias_float_not_int() {
    run_test("array/format_alias_float_not_int")
}

#[test]
fn format_alias_int_not_rational() {
    run_test("array/format_alias_int_not_rational")
}

#[test]
fn format_alias_not_numeric() {
    run_test("array/format_alias_not_numeric")
}

#[test]
fn format_alias_ok() {
    run_test("array/format_alias_ok")
}

#[test]
fn format_bad_syntax() {
    run_test("array/format_bad_syntax")
}

#[test]
fn format_float_not_int() {
    run_test("array/format_float_not_int")
}

#[test]
fn format_int_not_rational() {
    run_test("array/format_int_not_rational")
}

#[test]
fn format_missing_repl() {
    run_test("array/format_missing_repl")
}

#[test]
fn format_not_numeric() {
    run_test("array/format_not_numeric")
}

#[test]
fn format_numeric() {
    run_test("array/format_numeric")
}

#[test]
fn format_ok() {
    run_test("array/format_ok")
}

#[test]
fn format_precision_too_large() {
    run_test("array/format_precision_too_large")
}

#[test]
fn format_too_many_repls() {
    run_test("array/format_too_many_repls")
}

#[test]
fn invalid_size() {
    run_test("array/invalid_size")
}

#[test]
fn large_size() {
    run_test("array/large_size")
}

#[test]
fn no_default_ok() {
    run_test("array/no_default_ok")
}

#[test]
fn string_size_default_ok() {
    run_test("array/string_size_default_ok")
}

#[test]
fn struct_default_ok() {
    run_test("array/struct_default_ok")
}

#[test]
fn struct_no_default_ok() {
    run_test("array/struct_no_default_ok")
}

// Rust-only tests (no Scala counterpart)

/// A scalar default value is promoted to an array of arrays
#[test]
fn array_default_ok() {
    run_test("array/array_default_ok")
}

/// Default values of nested array types, including promoted and aliased elements
#[test]
fn nested_default_ok() {
    run_test("array/nested_default_ok")
}

/// The default value of a large nested array type is built without blowing up
#[test]
fn nested_default_large_ok() {
    run_test("array/nested_default_large_ok")
}

/// An array size outside the 32-bit range is an error
#[test]
fn size_out_of_range() {
    run_test("array/size_out_of_range")
}

/// A default value whose inner arrays have the wrong size
#[test]
fn default_nested_size_error() {
    run_test("array/default_nested_size_error")
}

/// A default value whose struct elements hold an array of the wrong size
#[test]
fn default_struct_element_size_error() {
    run_test("array/default_struct_element_size_error")
}

/// An array used by an earlier definition is checked first
#[test]
fn size_error_use_order() {
    run_test("array/size_error_use_order")
}

/// A format string error before a non-ASCII character
#[test]
fn format_bad_syntax_utf8() {
    run_test("array/format_bad_syntax_utf8")
}

/// A format string with an invalid replacement field after a valid one
#[test]
fn format_nul() {
    run_test("array/format_nul")
}

/// A format precision too large to represent
#[test]
fn format_precision_huge() {
    run_test("array/format_precision_huge")
}

/// A format string error in a multiline string literal
#[test]
fn format_multiline_bad_syntax() {
    run_test("array/format_multiline_bad_syntax")
}

/// A scalar default value is promoted to an alias of an array type
#[test]
fn default_alias_promote_ok() {
    run_test("array/default_alias_promote_ok")
}

/// The format error of an array type in a sizeof operand is reported once
#[test]
fn format_sizeof_error() {
    run_test("array/format_sizeof_error")
}
