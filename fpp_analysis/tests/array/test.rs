use crate::run_test;

#[test]
fn format_bad_syntax() {
    run_test("array/format_bad_syntax")
}

#[test]
fn enum_default_ok() {
    run_test("array/enum_default_ok")
}

#[test]
fn array_default_ok() {
    run_test("array/array_default_ok")
}

#[test]
fn format_precision_too_large() {
    run_test("array/format_precision_too_large")
}

#[test]
fn format_alias_ok() {
    run_test("array/format_alias_ok")
}

#[test]
fn format_int_not_rational() {
    run_test("array/format_int_not_rational")
}

#[test]
fn format_alias_not_numeric() {
    run_test("array/format_alias_not_numeric")
}

#[test]
fn default_error() {
    run_test("array/default_error")
}

#[test]
fn large_size() {
    run_test("array/large_size")
}

#[test]
fn enum_default_error() {
    run_test("array/enum_default_error")
}

#[test]
fn struct_default_ok() {
    run_test("array/struct_default_ok")
}

#[test]
fn struct_no_default_ok() {
    run_test("array/struct_no_default_ok")
}

#[test]
fn format_not_numeric() {
    run_test("array/format_not_numeric")
}

#[test]
fn invalid_size() {
    run_test("array/invalid_size")
}

#[test]
fn format_float_not_int() {
    run_test("array/format_float_not_int")
}

#[test]
fn format_missing_repl() {
    run_test("array/format_missing_repl")
}

#[test]
fn format_alias_float_not_int() {
    run_test("array/format_alias_float_not_int")
}

#[test]
fn format_too_many_repls() {
    run_test("array/format_too_many_repls")
}

#[test]
fn default_ok() {
    run_test("array/default_ok")
}

#[test]
fn nested_default_ok() {
    run_test("array/nested_default_ok")
}

#[test]
fn nested_default_large_ok() {
    run_test("array/nested_default_large_ok")
}

#[test]
fn format_alias_int_not_rational() {
    run_test("array/format_alias_int_not_rational")
}

#[test]
fn array_no_default_ok() {
    run_test("array/array_no_default_ok")
}

#[test]
fn format_ok() {
    run_test("array/format_ok")
}

#[test]
fn enum_no_default_ok() {
    run_test("array/enum_no_default_ok")
}

#[test]
fn format_numeric() {
    run_test("array/format_numeric")
}

#[test]
fn string_size_default_ok() {
    run_test("array/string_size_default_ok")
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
fn no_default_ok() {
    run_test("array/no_default_ok")
}

#[test]
fn size_out_of_range() {
    run_test("array/size_out_of_range")
}
