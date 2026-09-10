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

#[test]
fn div_by_near_zero_float() {
    run_test("expr/div_by_near_zero_float")
}

#[test]
fn add_ok() {
    run_test("expr/add_ok")
}

#[test]
fn arith_overflow_add() {
    run_test("expr/arith_overflow_add")
}

#[test]
fn arith_overflow_sub() {
    run_test("expr/arith_overflow_sub")
}

#[test]
fn arith_overflow_mul() {
    run_test("expr/arith_overflow_mul")
}

#[test]
fn arith_overflow_div() {
    run_test("expr/arith_overflow_div")
}

#[test]
fn arith_overflow_neg() {
    run_test("expr/arith_overflow_neg")
}

#[test]
fn arith_overflow_typed_context() {
    run_test("expr/arith_overflow_typed_context")
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

#[test]
fn div_by_zero_use_chain() {
    run_test("expr/div_by_zero_use_chain")
}

#[test]
fn shift_amount_use_chain() {
    run_test("expr/shift_amount_use_chain")
}

#[test]
fn subscript_use_chain() {
    run_test("expr/subscript_use_chain")
}

#[test]
fn binop_type_use_chain() {
    run_test("expr/binop_type_use_chain")
}

#[test]
fn arith_overflow_use_chain() {
    run_test("expr/arith_overflow_use_chain")
}
