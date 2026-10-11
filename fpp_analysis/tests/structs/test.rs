use crate::run_test;

#[test]
fn default_error() {
    run_test("structs/default_error")
}

#[test]
fn default_ok() {
    run_test("structs/default_ok")
}

#[test]
fn default_shadowed_error() {
    run_test("structs/default_shadowed_error")
}

#[test]
fn default_shadowed_ok() {
    run_test("structs/default_shadowed_ok")
}

#[test]
fn dictionary_not_displayable() {
    run_test("structs/dictionary_not_displayable")
}

#[test]
fn dictionary_ok() {
    run_test("structs/dictionary_ok")
}

#[test]
fn duplicate_names() {
    run_test("structs/duplicate_names")
}

#[test]
fn format_alias_not_numeric() {
    run_test("structs/format_alias_not_numeric")
}

#[test]
fn format_alias_numeric() {
    run_test("structs/format_alias_numeric")
}

#[test]
fn format_alias_ok() {
    run_test("structs/format_alias_ok")
}

#[test]
fn format_bad_syntax() {
    run_test("structs/format_bad_syntax")
}

#[test]
fn format_missing_repl() {
    run_test("structs/format_missing_repl")
}

#[test]
fn format_not_numeric() {
    run_test("structs/format_not_numeric")
}

#[test]
fn format_numeric() {
    run_test("structs/format_numeric")
}

#[test]
fn format_ok() {
    run_test("structs/format_ok")
}

#[test]
fn format_too_many_repls() {
    run_test("structs/format_too_many_repls")
}

#[test]
fn invalid_size() {
    run_test("structs/invalid_size")
}

#[test]
fn no_default_ok() {
    run_test("structs/no_default_ok")
}

#[test]
fn size_not_numeric() {
    run_test("structs/size_not_numeric")
}

// Rust-only tests (no Scala counterpart)

/// A default value whose member array has the wrong size
#[test]
fn default_member_size_error() {
    run_test("structs/default_member_size_error")
}

/// A member size error is reported before a member format error
#[test]
fn format_and_size_error() {
    run_test("structs/format_and_size_error")
}

/// A default value error is reported before a member size error
#[test]
fn default_and_size_error() {
    run_test("structs/default_and_size_error")
}

/// The definition of a member type is checked before the struct that uses it
#[test]
fn member_type_order() {
    run_test("structs/member_type_order")
}

/// A format precision out of range for a member
#[test]
fn format_precision_and_type() {
    run_test("structs/format_precision_and_type")
}

/// A member array size above the 32-bit range is an error
#[test]
fn size_out_of_range_high() {
    run_test("structs/size_out_of_range_high")
}

/// A member array size below the 32-bit range is an error
#[test]
fn size_out_of_range_low() {
    run_test("structs/size_out_of_range_low")
}

/// A scalar default member value is promoted to an alias of an array type
#[test]
fn default_alias_promote_ok() {
    run_test("structs/default_alias_promote_ok")
}

/// Duplicate members are reported before member type errors
#[test]
fn duplicate_member_before_member_types() {
    run_test("structs/duplicate_member_before_member_types")
}
