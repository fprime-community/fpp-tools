use crate::run_test;

#[test]
fn add_error() {
    run_test("expr/add_error")
}

#[test]
fn literal_ok() {
    run_test("expr/literal_ok")
}

#[test]
fn neg_error() {
    run_test("expr/neg_error")
}

#[test]
fn div_by_zero() {
    run_test("expr/div_by_zero")
}

/// A float divisor is tested for zero with an epsilon comparison, as
/// `Value.isZero` does, so a divisor nearer zero than `EPSILON` is a division by
/// zero even though the raw `f64` division would succeed.
#[test]
fn div_by_near_zero_float() {
    run_test("expr/div_by_near_zero_float")
}

#[test]
fn add_ok() {
    run_test("expr/add_ok")
}

/// Constant arithmetic whose exact result does not fit in an `i128`. Scala
/// evaluates these with `BigInt` and reports nothing; this port represents
/// values as `i128` and reports each inexpressible result rather than wrapping
/// it (a release build would wrap silently and a debug build would panic).
#[test]
fn arith_overflow_error() {
    run_test("expr/arith_overflow_error")
}

#[test]
fn array_error() {
    run_test("expr/array_error")
}

#[test]
fn neg_ok() {
    run_test("expr/neg_ok")
}

#[test]
fn array_empty() {
    run_test("expr/array_empty")
}

#[test]
fn dot_bad_expr() {
    run_test("expr/dot_bad_expr")
}

#[test]
fn paren_ok() {
    run_test("expr/paren_ok")
}

#[test]
fn struct_duplicate() {
    run_test("expr/struct_duplicate")
}

#[test]
fn array_ok() {
    run_test("expr/array_ok")
}

#[test]
fn sizeof_ok() {
    run_test("expr/sizeof_ok")
}

#[test]
fn sizeof_error() {
    run_test("expr/sizeof_error")
}

/// A serialized size that does not fit in an `i128`. Scala accumulates the size
/// in a `BigInt` and reports nothing; this port reports the inexpressible size
/// rather than wrapping it.
#[test]
fn sizeof_too_large() {
    run_test("expr/sizeof_too_large")
}

#[test]
fn sizeof_types() {
    run_test("expr/sizeof_types")
}

#[test]
fn sizeof_string() {
    run_test("expr/sizeof_string")
}

#[test]
fn sizeof_propagation() {
    run_test("expr/sizeof_propagation")
}

#[test]
fn sizeof_not_displayable() {
    run_test("expr/sizeof_not_displayable")
}

#[test]
fn sizeof_string_fw_store_type_not_defined() {
    run_test("expr/sizeof_string_fw_store_type_not_defined")
}

#[test]
fn string_concat_error() {
    run_test("expr/string_concat_error")
}

#[test]
fn string_concat_ok() {
    run_test("expr/string_concat_ok")
}

#[test]
fn binop_numeric_error() {
    run_test("expr/binop_numeric_error")
}

#[test]
fn subscript_order_error() {
    run_test("expr/subscript_order_error")
}

#[test]
fn string_size_sizeof_ok() {
    run_test("expr/string_size_sizeof_ok")
}

#[test]
fn string_size_sizeof_undefined() {
    run_test("expr/string_size_sizeof_undefined")
}

#[test]
fn subscript_index_too_large() {
    run_test("expr/subscript_index_too_large")
}
