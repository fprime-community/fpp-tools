use crate::run_test;

#[test]
fn enum_undef_type() {
    run_test("component/enum_undef_type")
}

#[test]
fn array_undef_type() {
    run_test("component/array_undef_type")
}

#[test]
fn struct_default_error() {
    run_test("component/struct_default_error")
}

#[test]
fn struct_alias_format_not_numeric() {
    run_test("component/struct_alias_format_not_numeric")
}

#[test]
fn enum_ok() {
    run_test("component/enum_ok")
}

#[test]
fn struct_format_not_numeric() {
    run_test("component/struct_format_not_numeric")
}

#[test]
fn undef_constant() {
    run_test("component/undef_constant")
}

#[test]
fn array_default_error() {
    run_test("component/array_default_error")
}

#[test]
fn enum_default_error() {
    run_test("component/enum_default_error")
}

#[test]
fn ok() {
    run_test("component/ok")
}

#[test]
fn struct_undef_type() {
    run_test("component/struct_undef_type")
}

#[test]
fn array_format_not_numeric() {
    run_test("component/array_format_not_numeric")
}

#[test]
fn array_alias_format_not_numeric() {
    run_test("component/array_alias_format_not_numeric")
}

#[test]
fn struct_undef_constant() {
    run_test("component/struct_undef_constant")
}

#[test]
fn abs_type_ok() {
    run_test("component/abs_type_ok")
}

#[test]
fn queued_no_async_input() {
    run_test("component/queued_no_async_input")
}

#[test]
fn active_no_async_input() {
    run_test("component/active_no_async_input")
}

#[test]
fn enum_undef_constant() {
    run_test("component/enum_undef_constant")
}

#[test]
fn struct_ok() {
    run_test("component/struct_ok")
}

#[test]
fn array_undef_constant() {
    run_test("component/array_undef_constant")
}

#[test]
fn array_ok() {
    run_test("component/array_ok")
}

#[test]
fn import_module() {
    run_test("component/import_module")
}

#[test]
fn passive_async_input_order() {
    run_test("component/passive_async_input_order")
}

#[test]
fn passive_async_command_order() {
    run_test("component/passive_async_command_order")
}
