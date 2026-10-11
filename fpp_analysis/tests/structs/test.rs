use crate::run_test;

#[test]
fn format_bad_syntax() {
    run_test("structs/format_bad_syntax")
}

#[test]
fn no_default_ok() {
    run_test("structs/no_default_ok")
}

#[test]
fn format_alias_ok() {
    run_test("structs/format_alias_ok")
}

#[test]
fn size_not_numeric() {
    run_test("structs/size_not_numeric")
}

#[test]
fn format_alias_not_numeric() {
    run_test("structs/format_alias_not_numeric")
}

#[test]
fn duplicate_names() {
    run_test("structs/duplicate_names")
}

#[test]
fn format_alias_numeric() {
    run_test("structs/format_alias_numeric")
}

#[test]
fn default_error() {
    run_test("structs/default_error")
}

#[test]
fn format_not_numeric() {
    run_test("structs/format_not_numeric")
}

#[test]
fn invalid_size() {
    run_test("structs/invalid_size")
}

#[test]
fn format_missing_repl() {
    run_test("structs/format_missing_repl")
}

#[test]
fn format_too_many_repls() {
    run_test("structs/format_too_many_repls")
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
fn format_ok() {
    run_test("structs/format_ok")
}

#[test]
fn format_numeric() {
    run_test("structs/format_numeric")
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
fn default_member_size_error() {
    run_test("structs/default_member_size_error")
}

#[test]
fn format_and_size_error() {
    run_test("structs/format_and_size_error")
}

#[test]
fn default_and_size_error() {
    run_test("structs/default_and_size_error")
}

#[test]
fn member_type_order() {
    run_test("structs/member_type_order")
}

#[test]
fn format_precision_and_type() {
    run_test("structs/format_precision_and_type")
}

#[test]
fn size_out_of_range_high() {
    run_test("structs/size_out_of_range_high")
}

#[test]
fn size_out_of_range_low() {
    run_test("structs/size_out_of_range_low")
}

#[test]
fn default_alias_promote_ok() {
    run_test("structs/default_alias_promote_ok")
}

#[test]
fn duplicate_member_before_member_types() {
    run_test("structs/duplicate_member_before_member_types")
}
