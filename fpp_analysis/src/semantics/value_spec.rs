#![cfg(test)]

use super::spec::*;
use crate::semantics::{
    AbsTypeValue, AnonArrayValue, AnonStructValue, ArrayValue, BooleanValue, EnumConstantValue,
    FloatValue, IntegerValue, MathError, MathResult, PrimitiveIntegerValue, StringValue,
    StructValue, Type, Value,
};
use indexmap::IndexMap;
use std::sync::Arc;

/// Structural equality on `Value` (which does not derive `PartialEq`): Scala's
/// case-class equality. Named types compare by definition.
///
/// Floats use exact `f64` equality because every test uses whole/half numbers.
fn values_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (
            Value::PrimitiveInteger(PrimitiveIntegerValue {
                value: v1,
                kind: k1,
            }),
            Value::PrimitiveInteger(PrimitiveIntegerValue {
                value: v2,
                kind: k2,
            }),
        ) => v1 == v2 && k1 == k2,
        (Value::Integer(IntegerValue(v1)), Value::Integer(IntegerValue(v2))) => v1 == v2,
        (
            Value::Float(FloatValue {
                value: v1,
                kind: k1,
            }),
            Value::Float(FloatValue {
                value: v2,
                kind: k2,
            }),
        ) => v1 == v2 && k1 == k2,
        (Value::Boolean(BooleanValue(v1)), Value::Boolean(BooleanValue(v2))) => v1 == v2,
        (Value::String(StringValue(v1)), Value::String(StringValue(v2))) => v1 == v2,
        (
            Value::EnumConstant(EnumConstantValue { value: v1, ty: t1 }),
            Value::EnumConstant(EnumConstantValue { value: v2, ty: t2 }),
        ) => v1 == v2 && t1.node.node_id == t2.node.node_id,
        (Value::AnonArray(a1), Value::AnonArray(a2)) => anon_arrays_eq(a1, a2),
        (
            Value::Array(ArrayValue {
                anon_array: a1,
                ty: t1,
            }),
            Value::Array(ArrayValue {
                anon_array: a2,
                ty: t2,
            }),
        ) => anon_arrays_eq(a1, a2) && t1.node.node_id == t2.node.node_id,
        (
            Value::AnonStruct(AnonStructValue { members: m1 }),
            Value::AnonStruct(AnonStructValue { members: m2 }),
        ) => members_eq(m1, m2),
        (
            Value::Struct(StructValue {
                anon_struct: AnonStructValue { members: m1 },
                ty: t1,
            }),
            Value::Struct(StructValue {
                anon_struct: AnonStructValue { members: m2 },
                ty: t2,
            }),
        ) => members_eq(m1, m2) && t1.node.node_id == t2.node.node_id,
        (Value::AbsType(AbsTypeValue { ty: t1 }), Value::AbsType(AbsTypeValue { ty: t2 })) => {
            t1.node.node_id == t2.node.node_id
        }
        _ => false,
    }
}

fn anon_arrays_eq(a1: &AnonArrayValue, a2: &AnonArrayValue) -> bool {
    let scalars_eq = match (&a1.scalar, &a2.scalar) {
        (Some(s1), Some(s2)) => values_eq(s1, s2),
        (None, None) => true,
        _ => false,
    };
    scalars_eq
        && a1.elements.len() == a2.elements.len()
        && a1
            .elements
            .iter()
            .zip(a2.elements.iter())
            .all(|(x, y)| values_eq(x, y))
}

fn members_eq(m1: &IndexMap<String, Value>, m2: &IndexMap<String, Value>) -> bool {
    m1.len() == m2.len()
        && m1
            .iter()
            .all(|(name, v1)| m2.get(name).is_some_and(|v2| values_eq(v1, v2)))
}

/// Drives a table of `(lhs, rhs, expected)` cases through a binary op.
/// `Some(v)` means `Ok` (checked with `values_eq`); `None` means that the
/// operation does not apply to the values.
#[track_caller]
fn check_binop(
    op_name: &str,
    op: impl Fn(&Value, &Value) -> MathResult,
    cases: Vec<(Value, Value, Option<Value>)>,
) {
    for (a, b, expected) in cases {
        let got = op(&a, &b);
        match (&got, &expected) {
            (Ok(g), Some(e)) => {
                assert!(values_eq(g, e), "{a} {op_name} {b}: expected {e}, got {g}")
            }
            (Err(MathError::InvalidInputs), None) => {}
            (Err(err), None) => panic!("{a} {op_name} {b}: expected InvalidInputs, got {err:?}"),
            (Ok(g), None) => panic!("{a} {op_name} {b}: expected error, got {g}"),
            (Err(_), Some(e)) => panic!("{a} {op_name} {b}: expected {e}, got error"),
        }
    }
}

/// Drives a table of `(value, type, expected)` cases through `Value::convert`.
#[track_caller]
fn check_convert(cases: Vec<(Value, Arc<Type>, Option<Value>)>) {
    for (value, ty, expected) in cases {
        let got = value.convert(&ty);
        match (&got, &expected) {
            (Some(g), Some(e)) => {
                assert!(
                    values_eq(g, e),
                    "convert {value} to {ty}: expected {e}, got {g}"
                )
            }
            (None, None) => {}
            (Some(g), None) => panic!("convert {value} to {ty}: expected None, got {g}"),
            (None, Some(e)) => panic!("convert {value} to {ty}: expected {e}, got None"),
        }
    }
}

// ---------------------------------------------------------------------------
// "getType" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_get_type() {
    with_test_ctx(|| {
        let pairs: Vec<(Value, Arc<Type>)> = vec![
            (default_i8(), i8()),
            (default_i16(), i16()),
            (default_i32(), i32()),
            (default_i64(), i64()),
            (default_u8(), u8()),
            (default_u16(), u16()),
            (default_u32(), u32()),
            (default_u64(), u64()),
            (default_integer(), integer()),
            (default_f32(), f32()),
            (default_f64(), f64()),
            (default_boolean(), boolean()),
            (default_string(), string(None)),
            (default_anon_array_3_u32(), default_anon_array_3_u32_type()),
            (array_value(), array_type()),
            (enumeration_value(), enum_type()),
            (anon_struct_value(), anon_struct_type()),
            (struct_value(), struct_type()),
        ];
        for (v, expected) in pairs {
            assert!(
                types_structurally_eq(&v.get_type(), &expected),
                "get_type({v}) expected `{expected}`, got `{}`",
                v.get_type()
            );
        }
    });
}

// ---------------------------------------------------------------------------
// "add" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_add() {
    with_test_ctx(|| {
        let cases: Vec<(Value, Value, Option<Value>)> = vec![
            (create_i8(1), create_i8(2), Some(create_i8(3))),
            (create_i8(1), create_integer(2), Some(create_integer(3))),
            (create_i8(1), create_f32(2.0), Some(create_f64(3.0))),
            (create_f32(1.0), create_f32(2.0), Some(create_f32(3.0))),
            (create_f32(1.0), create_f64(2.0), Some(create_f64(3.0))),
            (enumeration_value(), create_i32(1), Some(create_i32(1))),
            (enumeration_value(), create_i16(1), Some(create_integer(1))),
            (create_i32(1), create_string("abc"), None),
            (create_i32(1), default_anon_array_3_u32(), None),
            (create_i32(1), array_value(), None),
            (create_i32(1), anon_struct_value(), None),
            (create_i32(1), struct_value(), None),
            (
                create_string("abc"),
                create_string("def"),
                Some(create_string("abcdef")),
            ),
        ];
        check_binop("+", |a, b| a.add(b), cases);
    });
}

// ---------------------------------------------------------------------------
// "sub" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_sub() {
    with_test_ctx(|| {
        let cases: Vec<(Value, Value, Option<Value>)> = vec![
            (create_i8(1), create_i8(2), Some(create_i8(-1))),
            (create_i8(1), create_integer(2), Some(create_integer(-1))),
            (create_i8(1), create_f32(2.0), Some(create_f64(-1.0))),
            (create_f32(1.0), create_f32(2.0), Some(create_f32(-1.0))),
            (create_f32(1.0), create_f64(2.0), Some(create_f64(-1.0))),
            (enumeration_value(), create_i32(1), Some(create_i32(-1))),
            (enumeration_value(), create_i16(1), Some(create_integer(-1))),
            (create_i32(1), create_string("abc"), None),
            (create_i32(1), default_anon_array_3_u32(), None),
            (create_i32(1), array_value(), None),
            (create_i32(1), anon_struct_value(), None),
            (create_i32(1), struct_value(), None),
            (create_string("abc"), create_string("def"), None),
        ];
        check_binop("-", |a, b| a.sub(b), cases);
    });
}

// ---------------------------------------------------------------------------
// "mul" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_mul() {
    with_test_ctx(|| {
        let cases: Vec<(Value, Value, Option<Value>)> = vec![
            (create_i8(1), create_i8(2), Some(create_i8(2))),
            (create_i8(1), create_integer(2), Some(create_integer(2))),
            (create_i8(1), create_f32(2.0), Some(create_f64(2.0))),
            (create_f32(1.0), create_f32(2.0), Some(create_f32(2.0))),
            (create_f32(1.0), create_f64(2.0), Some(create_f64(2.0))),
            (enumeration_value(), create_i32(1), Some(create_i32(0))),
            (enumeration_value(), create_i16(1), Some(create_integer(0))),
            (create_i32(1), create_string("abc"), None),
            (create_i32(1), default_anon_array_3_u32(), None),
            (create_i32(1), array_value(), None),
            (create_i32(1), anon_struct_value(), None),
            (create_i32(1), struct_value(), None),
            (create_string("abc"), create_string("def"), None),
        ];
        check_binop("*", |a, b| a.mul(b), cases);
    });
}

// ---------------------------------------------------------------------------
// "div" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_div() {
    with_test_ctx(|| {
        let cases: Vec<(Value, Value, Option<Value>)> = vec![
            (create_i8(1), create_i8(2), Some(create_i8(0))),
            (create_i8(1), create_integer(2), Some(create_integer(0))),
            (create_i8(1), create_f32(2.0), Some(create_f64(0.5))),
            (create_f32(1.0), create_f32(2.0), Some(create_f32(0.5))),
            (create_f32(1.0), create_f64(2.0), Some(create_f64(0.5))),
            (enumeration_value(), create_i32(1), Some(create_i32(0))),
            (enumeration_value(), create_i16(1), Some(create_integer(0))),
            (create_i32(1), create_string("abc"), None),
            (create_i32(1), default_anon_array_3_u32(), None),
            (create_i32(1), array_value(), None),
            (create_i32(1), anon_struct_value(), None),
            (create_i32(1), struct_value(), None),
            (create_string("abc"), create_string("def"), None),
        ];
        check_binop("/", |a, b| a.div(b), cases);
    });
}

// ---------------------------------------------------------------------------
// "lshift" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_lshift() {
    with_test_ctx(|| {
        let cases: Vec<(Value, Value, Option<Value>)> = vec![
            (create_i8(1), create_i8(2), Some(create_i8(4))),
            (create_i32(1), create_i32(3), Some(create_i32(8))),
            (create_i8(-1), create_i8(1), Some(create_i8(-2))),
            (create_i32(1), create_integer(4), Some(create_integer(16))),
            (create_integer(1), create_i32(5), Some(create_integer(32))),
            (
                create_integer(1),
                create_integer(10),
                Some(create_integer(1024)),
            ),
            (enumeration_value(), create_i32(3), Some(create_i32(0))),
            (
                enumeration_value(),
                create_integer(3),
                Some(create_integer(0)),
            ),
            (create_i32(1), enumeration_value(), Some(create_i32(1))),
            (
                create_integer(1),
                enumeration_value(),
                Some(create_integer(1)),
            ),
            (create_i32(1), create_f32(2.0), None),
            (create_f32(1.0), create_i32(2), None),
            (create_i32(1), default_string(), None),
            (default_string(), create_i32(2), None),
            (create_i32(1), default_boolean(), None),
            (default_boolean(), create_i32(2), None),
            (create_i32(1), default_anon_array_3_u32(), None),
            (create_i32(1), array_value(), None),
            (create_i32(1), anon_struct_value(), None),
            (create_i32(1), struct_value(), None),
        ];
        check_binop("<<", |a, b| a.shl(b), cases);
    });
}

// ---------------------------------------------------------------------------
// "rshift" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_rshift() {
    with_test_ctx(|| {
        let cases: Vec<(Value, Value, Option<Value>)> = vec![
            (create_i8(8), create_i8(1), Some(create_i8(4))),
            (create_i32(16), create_i32(2), Some(create_i32(4))),
            (create_i8(-8), create_i8(1), Some(create_i8(-4))),
            (create_i32(16), create_integer(2), Some(create_integer(4))),
            (create_integer(32), create_i32(2), Some(create_integer(8))),
            (
                create_integer(64),
                create_integer(3),
                Some(create_integer(8)),
            ),
            (enumeration_value(), create_i32(1), Some(create_i32(0))),
            (
                enumeration_value(),
                create_integer(1),
                Some(create_integer(0)),
            ),
            (create_i32(16), enumeration_value(), Some(create_i32(16))),
            (
                create_integer(16),
                enumeration_value(),
                Some(create_integer(16)),
            ),
            (create_i32(16), create_f32(2.0), None),
            (create_f32(16.0), create_i32(2), None),
            (create_i32(16), default_string(), None),
            (default_string(), create_i32(2), None),
            (create_i32(16), default_boolean(), None),
            (default_boolean(), create_i32(2), None),
            (create_i32(16), default_anon_array_3_u32(), None),
            (create_i32(16), array_value(), None),
            (create_i32(16), anon_struct_value(), None),
            (create_i32(16), struct_value(), None),
        ];
        check_binop(">>", |a, b| a.shr(b), cases);
    });
}

// ---------------------------------------------------------------------------
// "is zero" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_is_zero() {
    with_test_ctx(|| {
        assert!(!create_i32(1).is_zero());
        assert!(create_i32(0).is_zero());
        // default_enum's "X" constant with value 0 is zero.
        assert!(enumeration_value().is_zero());
        assert!(!create_f32(1.0).is_zero());
        assert!(create_f32(0.0).is_zero());
        assert!(!array_value().is_zero());
        assert!(!default_anon_array_3_u32().is_zero());
        assert!(!struct_value().is_zero());
        assert!(!anon_struct_value().is_zero());
    });
}

// ---------------------------------------------------------------------------
// "convert to type" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_convert_to_type() {
    with_test_ctx(|| {
        // anon_struct_value() with the member c = default_i8() added
        let abc_struct = {
            let Value::AnonStruct(AnonStructValue { mut members }) = anon_struct_value() else {
                unreachable!("anon_struct_value is an anonymous struct value")
            };
            members.insert("c".to_string(), default_i8());
            Value::AnonStruct(AnonStructValue { members })
        };
        let cases: Vec<(Value, Arc<Type>, Option<Value>)> = vec![
            (create_i8(1), i8(), Some(create_i8(1))),
            (create_i8(1), i16(), Some(create_i16(1))),
            (create_i8(1), f32(), Some(create_f32(1.0))),
            (create_integer(1), i8(), Some(create_i8(1))),
            (create_i8(1), integer(), Some(create_integer(1))),
            (create_f64(1.0), integer(), Some(create_integer(1))),
            (create_f64(1.0), f32(), Some(create_f32(1.0))),
            (enumeration_value(), f32(), Some(create_f32(0.0))),
            (create_i32(0), enum_type(), None),
            (
                create_i32(42),
                default_anon_array_3_u32_type(),
                Some(create_anon_array(3, create_u32(42))),
            ),
            (anon_struct_value(), default_anon_array_3_u32_type(), None),
            (
                array_value(),
                default_anon_array_3_u32_type(),
                Some(default_anon_array_3_u32()),
            ),
            (anon_struct_value(), array_type(), None),
            (
                create_anon_array(2, create_i8(1)),
                default_anon_array_3_u32_type(),
                None,
            ),
            (
                anon_struct_value(),
                anon_struct_type(),
                Some(anon_struct_value()),
            ),
            (
                anon_struct_value(),
                anon_struct(&[("a", string(None))]),
                None,
            ),
            (
                anon_struct_value(),
                anon_struct(&[("a", u32()), ("b", string(None)), ("c", i8())]),
                Some(abc_struct.clone()),
            ),
            (
                struct_value(),
                anon_struct(&[("a", u32()), ("b", string(None)), ("c", i8())]),
                Some(abc_struct),
            ),
            (create_i32(0), anon_struct_type(), None),
            (
                create_i32(0),
                anon_struct(&[("a", u32())]),
                Some(Value::AnonStruct(AnonStructValue {
                    members: IndexMap::from([("a".to_string(), default_u32())]),
                })),
            ),
        ];
        check_convert(cases);
    });
}

// ---------------------------------------------------------------------------
// "negate" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_negate() {
    with_test_ctx(|| {
        assert!(values_eq(&create_i8(1).negate().unwrap(), &create_i8(-1)));
        assert!(values_eq(
            &create_f32(1.0).negate().unwrap(),
            &create_f32(-1.0)
        ));
        assert!(values_eq(
            &create_integer(1).negate().unwrap(),
            &create_integer(-1)
        ));
        assert!(default_string().negate().is_err());
        assert!(default_anon_array_3_u32().negate().is_err());
        assert!(anon_struct_value().negate().is_err());
        // Enum constant negates through its rep type (default_enum rep is I32).
        assert!(values_eq(
            &create_enum_constant("X", 1).negate().unwrap(),
            &create_i32(-1)
        ));
    });
}

// ---------------------------------------------------------------------------
// "truncate" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_truncate() {
    with_test_ctx(|| {
        // Scalar truncation wraps modulo the type width.
        assert!(values_eq(&create_u8(256).truncate(), &create_u8(0)));
        assert!(values_eq(&create_i8(256).truncate(), &create_i8(0)));
        assert!(values_eq(&create_u8(257).truncate(), &create_u8(1)));
        assert!(values_eq(&create_i8(257).truncate(), &create_i8(1)));
        assert!(values_eq(&create_u8(-1).truncate(), &create_u8(255)));

        // Array truncation is elementwise.
        assert!(values_eq(
            &create_anon_array(3, create_u8(256)).truncate(),
            &create_anon_array(3, create_u8(0))
        ));
        assert!(values_eq(
            &create_anon_array(3, create_i8(256)).truncate(),
            &create_anon_array(3, create_i8(0))
        ));
        assert!(values_eq(
            &create_anon_array(3, create_u8(257)).truncate(),
            &create_anon_array(3, create_u8(1))
        ));
        assert!(values_eq(
            &create_anon_array(3, create_i8(257)).truncate(),
            &create_anon_array(3, create_i8(1))
        ));
        assert!(values_eq(
            &create_anon_array(3, create_u8(-1)).truncate(),
            &create_anon_array(3, create_u8(255))
        ));
    });
}

// ---------------------------------------------------------------------------
// Rust-only
// ---------------------------------------------------------------------------

/// A primitive integer shifted by a primitive integer of another kind keeps its
/// own kind
#[test]
fn value_shift_keeps_the_left_kind() {
    with_test_ctx(|| {
        check_binop(
            "<<",
            |a, b| a.shl(b),
            vec![(create_i8(1), create_i32(2), Some(create_i8(4)))],
        );
        check_binop(
            ">>",
            |a, b| a.shr(b),
            vec![(create_i8(16), create_i32(2), Some(create_i8(4)))],
        );
    });
}

/// An arbitrary-precision right shift by an amount at or beyond the width of the
/// value saturates to `0` or `-1`, which an `i128` shift reproduces.
#[test]
fn value_rshift_saturates_past_the_value_width() {
    with_test_ctx(|| {
        let cases: Vec<(Value, Value, Option<Value>)> = vec![
            (
                create_integer(2),
                create_integer(200),
                Some(create_integer(0)),
            ),
            (
                create_integer(-2),
                create_integer(200),
                Some(create_integer(-1)),
            ),
            (
                create_integer(2),
                create_integer(255),
                Some(create_integer(0)),
            ),
            (create_i32(-1), create_i32(255), Some(create_i32(-1))),
        ];
        check_binop(">>", |a, b| a.shr(b), cases);
    });
}

/// A left shift whose exact result does not fit in an `i128` is reported rather
/// than silently wrapped.
#[test]
fn value_lshift_reports_overflow() {
    with_test_ctx(|| {
        // `-1 << 127` is exactly `i128::MIN`, so it is still representable
        assert!(matches!(
            create_integer(-1).shl(&create_integer(127)),
            Ok(Value::Integer(IntegerValue(i128::MIN)))
        ));
        // Shifting zero is zero, however large the amount
        assert!(matches!(
            create_integer(0).shl(&create_integer(255)),
            Ok(Value::Integer(IntegerValue(0)))
        ));
        for amount in [127, 128, 200, 255] {
            assert!(
                matches!(
                    create_integer(1).shl(&create_integer(amount)),
                    Err(MathError::ShiftOverflow)
                ),
                "1 << {amount} should overflow"
            );
        }
    });
}

#[test]
fn value_arithmetic_reports_overflow() {
    with_test_ctx(|| {
        let max = create_integer(i128::MAX);
        let min = create_integer(i128::MIN);

        // add / sub / mul at either end of the range
        assert!(matches!(
            max.add(&create_integer(1)),
            Err(MathError::Overflow)
        ));
        assert!(matches!(
            min.add(&create_integer(-1)),
            Err(MathError::Overflow)
        ));
        assert!(matches!(
            min.sub(&create_integer(1)),
            Err(MathError::Overflow)
        ));
        assert!(matches!(
            max.sub(&create_integer(-1)),
            Err(MathError::Overflow)
        ));
        assert!(matches!(
            max.mul(&create_integer(2)),
            Err(MathError::Overflow)
        ));
        assert!(matches!(
            min.mul(&create_integer(-1)),
            Err(MathError::Overflow)
        ));

        // `i128::MIN / -1` is the one integer division with an out-of-range result
        assert!(matches!(
            min.div(&create_integer(-1)),
            Err(MathError::Overflow)
        ));

        // Negation of the one value with no positive counterpart
        assert!(matches!(min.negate(), Err(MathError::Overflow)));

        // The sized-integer arms overflow the same way, whatever the kind
        assert!(matches!(
            create_i8(i128::MAX).add(&create_i8(1)),
            Err(MathError::Overflow)
        ));
        assert!(matches!(
            create_u32(i128::MIN).negate(),
            Err(MathError::Overflow)
        ));
        assert!(matches!(
            create_enum_constant("X", i128::MIN).negate(),
            Err(MathError::Overflow)
        ));

        // Results that do fit are still computed exactly
        assert!(values_eq(
            &max.sub(&create_integer(1)).unwrap(),
            &create_integer(i128::MAX - 1)
        ));
        assert!(values_eq(
            &min.div(&create_integer(2)).unwrap(),
            &create_integer(-(1 << 126))
        ));
        assert!(values_eq(
            &create_integer(i128::MIN + 1).negate().unwrap(),
            &create_integer(i128::MAX)
        ));
    });
}

/// Floats saturate at infinity instead of overflowing, so no arithmetic on them
/// is reported.
#[test]
fn value_float_arithmetic_saturates() {
    with_test_ctx(|| {
        match create_f64(f64::MAX).mul(&create_f64(2.0)) {
            Ok(Value::Float(FloatValue { value, .. })) => assert!(value.is_infinite()),
            other => panic!("expected an infinite float, got {other:?}"),
        }
        match create_f64(f64::MIN).add(&create_f64(f64::MIN)) {
            Ok(Value::Float(FloatValue { value, .. })) => {
                assert!(value.is_infinite() && value.is_sign_negative())
            }
            other => panic!("expected -inf, got {other:?}"),
        }
    });
}

/// A divisor is tested for zero before the operator is applied, using
/// `Value::is_zero`, so a float nearer zero than `EPSILON` is a division by
/// zero.
#[test]
fn value_div_by_zero_uses_is_zero() {
    with_test_ctx(|| {
        for divisor in [
            create_integer(0),
            create_i32(0),
            create_enum_constant("X", 0),
        ] {
            assert!(matches!(
                create_integer(1).div(&divisor),
                Err(MathError::DivByZero)
            ));
            assert!(matches!(
                create_f64(1.0).div(&divisor),
                Err(MathError::DivByZero)
            ));
        }
        // Nearer zero than EPSILON = 1e-7
        assert!(matches!(
            create_f64(1.0).div(&create_f64(1e-8)),
            Err(MathError::DivByZero)
        ));
        assert!(matches!(
            create_integer(1).div(&create_f64(-1e-8)),
            Err(MathError::DivByZero)
        ));
        // Just outside EPSILON: an ordinary division
        assert!(create_f64(1.0).div(&create_f64(1e-6)).is_ok());
    });
}

/// A scalar promotes to a *named* array/struct target, yielding a `Value::Array`
/// / `Value::Struct` (the promotion arms that build named aggregates).
#[test]
fn value_convert_scalar_to_named_aggregates() {
    with_test_ctx(|| {
        // Scalar -> named array `A = [2] U32`.
        let arr_ty = array("A", anon_array(Some(2), u32()), 500);
        match create_i32(7).convert(&arr_ty) {
            Some(Value::Array(a)) => {
                assert_eq!(a.anon_array.elements.len(), 2);
                assert!(
                    a.anon_array
                        .elements
                        .iter()
                        .all(|e| values_eq(e, &create_u32(7)))
                );
            }
            other => panic!("expected Value::Array, got {other:?}"),
        }

        // Scalar -> named struct `S { a: U32, b: U32 }`.
        let s_ty = struct_("S", anon_struct(&[("a", u32()), ("b", u32())]), 501, &[]);
        match create_i32(3).convert(&s_ty) {
            Some(Value::Struct(s)) => {
                assert_eq!(s.anon_struct.members.len(), 2);
                assert!(values_eq(
                    s.anon_struct.members.get("a").unwrap(),
                    &create_u32(3)
                ));
                assert!(values_eq(
                    s.anon_struct.members.get("b").unwrap(),
                    &create_u32(3)
                ));
            }
            other => panic!("expected Value::Struct, got {other:?}"),
        }
    });
}

/// Float-value conversions: `F64 -> {I32, F32, Integer}` and a non-numeric
/// target (`bool`) which fails.
#[test]
fn value_convert_float_source() {
    with_test_ctx(|| {
        assert!(values_eq(
            &create_f64(2.0).convert(&i32()).unwrap(),
            &create_i32(2)
        ));
        assert!(values_eq(
            &create_f64(2.0).convert(&f32()).unwrap(),
            &create_f32(2.0)
        ));
        assert!(values_eq(
            &create_f64(2.0).convert(&integer()).unwrap(),
            &create_integer(2)
        ));
        assert!(create_f64(2.0).convert(&boolean()).is_none());
    });
}

#[test]
fn value_convert_float_narrows_to_i32_width() {
    with_test_ctx(|| {
        const I32_MAX: i128 = i32::MAX as i128;
        const I32_MIN: i128 = i32::MIN as i128;

        // Saturating, not wrapping: the low bits of a wider result would make
        // `[1] I32 default [1.0e300]` come out as -1
        assert!(values_eq(
            &create_f64(1.0e300).convert(&i32()).unwrap(),
            &create_i32(I32_MAX)
        ));
        assert!(values_eq(
            &create_f64(-1.0e300).convert(&i32()).unwrap(),
            &create_i32(I32_MIN)
        ));

        // A wider target kind does not widen the conversion
        assert!(values_eq(
            &create_f64(1.0e18).convert(&i64()).unwrap(),
            &create_i64(I32_MAX)
        ));
        assert!(values_eq(
            &create_f64(1.0e18).convert(&integer()).unwrap(),
            &create_integer(I32_MAX)
        ));

        // NaN and the infinities land at zero and the ends of the range
        assert!(values_eq(
            &create_f64(f64::NAN).convert(&i32()).unwrap(),
            &create_i32(0)
        ));
        assert!(values_eq(
            &create_f64(f64::INFINITY).convert(&integer()).unwrap(),
            &create_integer(I32_MAX)
        ));
        assert!(values_eq(
            &create_f64(f64::NEG_INFINITY).convert(&integer()).unwrap(),
            &create_integer(I32_MIN)
        ));

        // Values inside the range round towards zero
        assert!(values_eq(
            &create_f64(2.9).convert(&i32()).unwrap(),
            &create_i32(2)
        ));
        assert!(values_eq(
            &create_f64(-2.9).convert(&i32()).unwrap(),
            &create_i32(-2)
        ));
    });
}

/// A bool converts to `bool` and nothing else; a string converts to any string.
#[test]
fn value_convert_bool_and_string() {
    with_test_ctx(|| {
        assert!(values_eq(
            &create_boolean(true).convert(&boolean()).unwrap(),
            &create_boolean(true)
        ));
        assert!(create_boolean(true).convert(&i32()).is_none());

        assert!(values_eq(
            &create_string("hi").convert(&string(Some(8))).unwrap(),
            &create_string("hi")
        ));
        assert!(create_string("hi").convert(&boolean()).is_none());
    });
}

/// A value whose type is a named definition converts to that same definition
/// via the identity fast path (no structural rebuild).
#[test]
fn value_convert_named_identity() {
    with_test_ctx(|| {
        // Same enum definition id -> identity conversion succeeds.
        let e = enumeration("E", fpp_ast::IntegerKind::I32, 510);
        let ec = Value::EnumConstant(EnumConstantValue::new("X".to_string(), 1, as_enum_ty(&e)));
        assert!(ec.convert(&e).is_some());
    });
}

/// `Integer`-left and `Float`-left arithmetic arms (mixed operands).
#[test]
fn value_binop_integer_and_float_left() {
    with_test_ctx(|| {
        // (lhs, rhs, expected) using the existing `check_binop` (Some=Ok, None=Err).
        let cases: Vec<(Value, Value, Option<Value>)> = vec![
            // Integer-left arm
            (create_integer(1), create_f32(2.0), Some(create_f64(3.0))),
            (
                create_integer(1),
                create_integer(2),
                Some(create_integer(3)),
            ),
            (
                create_integer(4),
                create_enum_constant("X", 1),
                Some(create_integer(5)),
            ),
            (create_integer(1), create_string("x"), None),
            // Float-left arm
            (create_f64(1.0), create_integer(2), Some(create_f64(3.0))),
            (create_f32(1.0), create_f64(2.0), Some(create_f64(3.0))),
            (create_f64(1.0), create_string("x"), None),
            // enum-left arm with matching rep type -> keeps the primitive kind
            (
                create_enum_constant("X", 2),
                create_i32(3),
                Some(create_i32(5)),
            ),
        ];
        check_binop("+", |a, b| a.add(b), cases);

        // Integer / 0 -> DivByZero (Value::div tests the divisor with is_zero)
        assert!(matches!(
            create_integer(1).div(&create_integer(0)),
            Err(MathError::DivByZero)
        ));
    });
}

/// An array default holds `size` references to one element value, so a
/// nested array default costs the SUM of the nested sizes and not their
/// PRODUCT.
///
/// The pointer-identity assertions are what bound the memory: with an
/// independent copy per element, `array Inner = [n] U8` +
/// `array Outer = [n] Inner` would build n^2 values, which at the size bound
/// (n = 2^31-1) cannot complete.
#[test]
fn array_default_shares_its_repeated_element() {
    const N: usize = 2000;

    let inner = anon_array(Some(N), u8());
    let outer = anon_array(Some(N), inner);

    let Some(Value::AnonArray(outer_default)) = outer.default_value() else {
        panic!("expected an anonymous array default");
    };
    assert_eq!(outer_default.elements.len(), N);

    // Every element of the outer array is the one shared inner array
    let first = &outer_default.elements[0];
    assert!(
        outer_default.elements.iter().all(|e| Arc::ptr_eq(e, first)),
        "the outer array's elements are not shared"
    );

    // ... and that inner array shares its own repeated element in turn, so
    // the whole default holds 2N references to 2 distinct values
    let Value::AnonArray(inner_default) = first.as_ref() else {
        panic!("expected an anonymous array element");
    };
    assert_eq!(inner_default.elements.len(), N);
    let inner_first = &inner_default.elements[0];
    assert!(
        inner_default
            .elements
            .iter()
            .all(|e| Arc::ptr_eq(e, inner_first)),
        "the inner array's elements are not shared"
    );
    assert!(values_eq(inner_first, &create_u8(0)));

    // Reading the value is unaffected by the sharing
    assert_eq!(inner_default.iter().count(), N);
    assert!(inner_default.iter().all(|e| e.to_string() == "0: U8"));
    assert!(inner_default.get(N - 1).is_some());
    assert!(inner_default.get(N).is_none());
}

/// A scalar promoted to an array of known size shares the promoted element
/// too, and an array of unknown size records it as the array's scalar.
#[test]
fn promoted_scalar_shares_its_element() {
    const N: usize = 1000;
    let scalar = create_u32(7);

    let u32_array = |size| anon_array(size, u32());
    let Some(Value::AnonArray(promoted)) = scalar.convert(&u32_array(Some(N))) else {
        panic!("expected an anonymous array");
    };
    assert_eq!(promoted.elements.len(), N);
    let first = &promoted.elements[0];
    assert!(promoted.elements.iter().all(|e| Arc::ptr_eq(e, first)));

    // Unknown size: no elements, and the value is kept as the scalar
    let Some(Value::AnonArray(unsized_array)) = scalar.convert(&u32_array(None)) else {
        panic!("expected an anonymous array");
    };
    assert!(unsized_array.elements.is_empty());
    assert!(unsized_array.scalar.is_some());
}

/// Converting and truncating an array preserve the sharing, so neither turns
/// a shared default back into one copy per element.
#[test]
fn array_conversion_preserves_sharing() {
    const N: usize = 2000;
    let u8_array = anon_array(Some(N), u8());
    let u32_array = anon_array(Some(N), u32());
    let value = u8_array.default_value().expect("an array default");

    for converted in [
        value.convert(&u32_array),
        value.truncate().convert(&u8_array),
        Some(value.truncate()),
    ] {
        let Some(Value::AnonArray(a)) = converted else {
            panic!("expected an anonymous array");
        };
        assert_eq!(a.elements.len(), N);
        let first = &a.elements[0];
        assert!(
            a.elements.iter().all(|e| Arc::ptr_eq(e, first)),
            "sharing was not preserved"
        );
    }
}

/// `Display` for every `Value` variant: the Scala `Value.toString`.
#[test]
fn value_display() {
    with_test_ctx(|| {
        assert_eq!(create_i8(5).to_string(), "5: I8");
        assert_eq!(create_u8(200).to_string(), "200: U8");
        assert_eq!(create_integer(-3).to_string(), "-3");
        assert_eq!(create_f64(1.5).to_string(), "1.5: F64");
        assert_eq!(create_f64(2.0).to_string(), "2.0: F64");
        assert_eq!(
            create_f32(0.1_f32 as f64).to_string(),
            "0.10000000149011612: F32"
        );
        assert_eq!(create_f64(1.0e300).to_string(), "1.0E300: F64");
        assert_eq!(create_f64(1.5e-5).to_string(), "1.5E-5: F64");
        assert_eq!(create_f64(0.001).to_string(), "0.001: F64");
        assert_eq!(create_f64(1234567.0).to_string(), "1234567.0: F64");
        assert_eq!(create_f64(12345678.0).to_string(), "1.2345678E7: F64");
        assert_eq!(create_f64(-0.5).to_string(), "-0.5: F64");
        assert_eq!(create_boolean(true).to_string(), "true");
        assert_eq!(create_string("hi").to_string(), "\"hi\"");
        assert_eq!(create_enum_constant("X", 7).to_string(), "(X,7): E");
        // An enum constant wider than 32 bits displays in full
        assert_eq!(
            create_enum_constant("X", 0xFFFFFFFFFFFFFFFF_u64 as i128).to_string(),
            "(X,18446744073709551615): E"
        );
        assert_eq!(
            create_anon_array(2, create_u32(0)).to_string(),
            "[ 0: U32, 0: U32 ]"
        );
        assert_eq!(
            Value::AnonArray(AnonArrayValue::promoted(create_u32(1))).to_string(),
            "[ 1: U32, ... ]"
        );
        assert_eq!(array_value().to_string(), "[ 0: I32, 0: I32, 0: I32 ]: A");
        assert_eq!(anon_struct_value().to_string(), "{ a = 0: U32, b = \"\" }");
        assert_eq!(
            Value::AnonStruct(AnonStructValue {
                members: IndexMap::new()
            })
            .to_string(),
            "{ }"
        );
        assert_eq!(struct_value().to_string(), "{ a = 0: U32, b = \"\" }: S");
    });
}
