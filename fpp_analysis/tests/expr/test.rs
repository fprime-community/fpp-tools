use crate::run_test;

#[test]
fn add_error() {
    run_test("expr/add_error")
}

#[test]
fn array_empty() {
    run_test("expr/array_empty")
}

#[test]
fn array_error() {
    run_test("expr/array_error")
}

#[test]
fn array_ok() {
    run_test("expr/array_ok")
}

#[test]
fn div_by_zero() {
    run_test("expr/div_by_zero")
}

#[test]
fn dot_bad_expr() {
    run_test("expr/dot_bad_expr")
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
fn neg_ok() {
    run_test("expr/neg_ok")
}

#[test]
fn paren_ok() {
    run_test("expr/paren_ok")
}

#[test]
fn sizeof_alias_alias_struct_ok() {
    run_test("expr/sizeof_alias_alias_struct_ok")
}

#[test]
fn sizeof_alias_struct_ok() {
    run_test("expr/sizeof_alias_struct_ok")
}

#[test]
fn sizeof_not_displayable() {
    run_test("expr/sizeof_not_displayable")
}

#[test]
fn sizeof_ok() {
    run_test("expr/sizeof_ok")
}

#[test]
fn sizeof_string_fw_store_type_not_defined() {
    run_test("expr/sizeof_string_fw_store_type_not_defined")
}

#[test]
fn struct_duplicate() {
    run_test("expr/struct_duplicate")
}

#[test]
fn string_concat_ok() {
    run_test("expr/string_concat_ok")
}

#[test]
fn string_concat_error() {
    run_test("expr/string_concat_error")
}

// Rust-only tests (no Scala counterpart)

/// Negating an enum constant yields type Integer
#[test]
fn neg_enum_error() {
    run_test("expr/neg_enum_error")
}

/// Division by a floating-point value close to zero
#[test]
fn div_by_near_zero_float() {
    run_test("expr/div_by_near_zero_float")
}

/// Adding two integer literals
#[test]
fn add_ok() {
    run_test("expr/add_ok")
}

/// Adding two enum constants yields type Integer
#[test]
fn add_enum_error() {
    run_test("expr/add_enum_error")
}

/// An addition past the 128-bit integer range is an error
#[test]
fn arith_overflow_add() {
    run_test("expr/arith_overflow_add")
}

/// A subtraction past the 128-bit integer range is an error
#[test]
fn arith_overflow_sub() {
    run_test("expr/arith_overflow_sub")
}

/// A multiplication past the 128-bit integer range is an error
#[test]
fn arith_overflow_mul() {
    run_test("expr/arith_overflow_mul")
}

/// A division past the 128-bit integer range is an error
#[test]
fn arith_overflow_div() {
    run_test("expr/arith_overflow_div")
}

/// A negation past the 128-bit integer range is an error
#[test]
fn arith_overflow_neg() {
    run_test("expr/arith_overflow_neg")
}

/// An overflow in an array default is reported before the conversion
#[test]
fn arith_overflow_typed_context() {
    run_test("expr/arith_overflow_typed_context")
}

/// Negating an enum constant yields type Integer
#[test]
fn neg_enum_ok() {
    run_test("expr/neg_enum_ok")
}

/// Selecting a member that a struct type does not have
#[test]
fn dot_struct_member_error() {
    run_test("expr/dot_struct_member_error")
}

/// Selecting a member that the anonymous struct value of a constant does not have
#[test]
fn dot_anon_struct_member_error() {
    run_test("expr/dot_anon_struct_member_error")
}

/// Selecting a member that an anonymous struct expression does not have
#[test]
fn dot_anon_struct_literal_member_error() {
    run_test("expr/dot_anon_struct_literal_member_error")
}

/// Duplicate members are checked before the member values are typed
#[test]
fn struct_duplicate_order_error() {
    run_test("expr/struct_duplicate_order_error")
}

/// The size of an abstract type is not known
#[test]
fn sizeof_error() {
    run_test("expr/sizeof_error")
}

/// A serialized size too large to represent is an error
#[test]
fn sizeof_too_large() {
    run_test("expr/sizeof_too_large")
}

/// The sizes of array, struct and enum types
#[test]
fn sizeof_types() {
    run_test("expr/sizeof_types")
}

/// The sizes of string types with default and explicit sizes
#[test]
fn sizeof_string() {
    run_test("expr/sizeof_string")
}

/// A sizeof value flows into an enum constant
#[test]
fn sizeof_propagation() {
    run_test("expr/sizeof_propagation")
}

/// Subtracting Boolean values is an error
#[test]
fn binop_numeric_error() {
    run_test("expr/binop_numeric_error")
}

/// The sum of two enum constants has no common type with a string
#[test]
fn binop_enum_common_type_error() {
    run_test("expr/binop_enum_common_type_error")
}

/// The array operand of a subscript is checked before the index
#[test]
fn subscript_order_error() {
    run_test("expr/subscript_order_error")
}

/// The array operand of a subscript is checked before the index is typed
#[test]
fn subscript_index_common_type_order_error() {
    run_test("expr/subscript_index_common_type_order_error")
}

/// The array operand of a subscript is checked before the index is typed
#[test]
fn subscript_index_numeric_order_error() {
    run_test("expr/subscript_index_numeric_order_error")
}

/// String sizes given by sizeof expressions and constants
#[test]
fn string_size_sizeof_ok() {
    run_test("expr/string_size_sizeof_ok")
}

/// A string size whose sizeof operand is undefined
#[test]
fn string_size_sizeof_undefined() {
    run_test("expr/string_size_sizeof_undefined")
}

/// A subscript index too large to represent is an error
#[test]
fn subscript_index_too_large() {
    run_test("expr/subscript_index_too_large")
}

/// A division by zero is reported once, however often the constant is used
#[test]
fn div_by_zero_use_chain() {
    run_test("expr/div_by_zero_use_chain")
}

/// An invalid shift amount is reported once, however often the constant is used
#[test]
fn shift_amount_use_chain() {
    run_test("expr/shift_amount_use_chain")
}

/// An out-of-range subscript is reported once, however often the constant is used
#[test]
fn subscript_use_chain() {
    run_test("expr/subscript_use_chain")
}

/// A type error is reported once, however often the constant is used
#[test]
fn binop_type_use_chain() {
    run_test("expr/binop_type_use_chain")
}

/// An arithmetic overflow is reported once, however often the constant is used
#[test]
fn arith_overflow_use_chain() {
    run_test("expr/arith_overflow_use_chain")
}

/// A negative string size is an error in a sizeof operand
#[test]
fn sizeof_string_size_negative() {
    run_test("expr/sizeof_string_size_negative")
}

/// A string size of 2^31 or more is an error in a sizeof operand
#[test]
fn sizeof_string_size_too_large() {
    run_test("expr/sizeof_string_size_too_large")
}

/// A sizeof operand may be a type parameter bound to a string type
#[test]
fn sizeof_string_size_template_ok() {
    run_test("expr/sizeof_string_size_template_ok")
}
