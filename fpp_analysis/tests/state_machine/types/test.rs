use crate::run_test;

#[test]
fn abs_type_ok() {
    run_test("state_machine/types/abs_type_ok")
}

#[test]
fn action_undef_type() {
    run_test("state_machine/types/action_undef_type")
}

#[test]
fn array_alias_format_not_numeric() {
    run_test("state_machine/types/array_alias_format_not_numeric")
}

#[test]
fn array_default_error() {
    run_test("state_machine/types/array_default_error")
}

#[test]
fn array_format_not_numeric() {
    run_test("state_machine/types/array_format_not_numeric")
}

#[test]
fn array_ok() {
    run_test("state_machine/types/array_ok")
}

#[test]
fn array_undef_constant() {
    run_test("state_machine/types/array_undef_constant")
}

#[test]
fn array_undef_type() {
    run_test("state_machine/types/array_undef_type")
}

#[test]
fn enum_default_error() {
    run_test("state_machine/types/enum_default_error")
}

#[test]
fn enum_ok() {
    run_test("state_machine/types/enum_ok")
}

#[test]
fn enum_undef_constant() {
    run_test("state_machine/types/enum_undef_constant")
}

#[test]
fn enum_undef_type() {
    run_test("state_machine/types/enum_undef_type")
}

#[test]
fn guard_undef_type() {
    run_test("state_machine/types/guard_undef_type")
}

#[test]
fn shadowed_error() {
    run_test("state_machine/types/shadowed_error")
}

#[test]
fn signal_undef_type() {
    run_test("state_machine/types/signal_undef_type")
}

#[test]
fn state_enum_ok() {
    run_test("state_machine/types/state_enum_ok")
}

#[test]
fn struct_alias_format_not_numeric() {
    run_test("state_machine/types/struct_alias_format_not_numeric")
}

#[test]
fn struct_default_error() {
    run_test("state_machine/types/struct_default_error")
}

#[test]
fn struct_format_not_numeric() {
    run_test("state_machine/types/struct_format_not_numeric")
}

#[test]
fn struct_ok() {
    run_test("state_machine/types/struct_ok")
}

#[test]
fn struct_undef_constant() {
    run_test("state_machine/types/struct_undef_constant")
}

#[test]
fn struct_undef_type() {
    run_test("state_machine/types/struct_undef_type")
}
