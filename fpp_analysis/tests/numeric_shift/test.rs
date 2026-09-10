use crate::run_test;

#[test]
fn shift_ok() {
    run_test("numeric_shift/shift_ok")
}

#[test]
fn shift_amount_too_large_error() {
    run_test("numeric_shift/shift_amount_too_large_error")
}

#[test]
fn shift_boolean_value_error() {
    run_test("numeric_shift/shift_boolean_value_error")
}

#[test]
fn shift_float_amount_error() {
    run_test("numeric_shift/shift_float_amount_error")
}

#[test]
fn shift_float_value_error() {
    run_test("numeric_shift/shift_float_value_error")
}

#[test]
fn shift_invalid_type_error() {
    run_test("numeric_shift/shift_invalid_type_error")
}

#[test]
fn shift_negative_amount_error() {
    run_test("numeric_shift/shift_negative_amount_error")
}

#[test]
fn shift_negative_rshift_error() {
    run_test("numeric_shift/shift_negative_rshift_error")
}

#[test]
fn shift_wide_amount_positive_error() {
    run_test("numeric_shift/shift_wide_amount_positive_error")
}

#[test]
fn shift_wide_amount_negative_error() {
    run_test("numeric_shift/shift_wide_amount_negative_error")
}

#[test]
fn shift_overflow_error() {
    run_test("numeric_shift/shift_overflow_error")
}
