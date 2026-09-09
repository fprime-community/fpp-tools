#![cfg(test)]

use super::test_helpers::*;
use crate::semantics::{
    AnonArrayValue, AnonStructValue, ArrayValue, BooleanValue, EnumConstantValue, FloatValue,
    IntegerValue, MathResult, PrimitiveIntegerValue, StringValue, StructValue, Type, Value,
};
use rustc_hash::FxHashMap;
use std::sync::Arc;

/// Structural equality on `Value` (which does not derive `PartialEq`).
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
            Value::EnumConstant(EnumConstantValue { value: v1, .. }),
            Value::EnumConstant(EnumConstantValue { value: v2, .. }),
        ) => v1 == v2,
        (
            Value::AnonArray(AnonArrayValue { elements: e1 }),
            Value::AnonArray(AnonArrayValue { elements: e2 }),
        )
        | (
            Value::Array(ArrayValue {
                anon_array: AnonArrayValue { elements: e1 },
                ..
            }),
            Value::Array(ArrayValue {
                anon_array: AnonArrayValue { elements: e2 },
                ..
            }),
        ) => e1.len() == e2.len() && e1.iter().zip(e2.iter()).all(|(x, y)| values_eq(x, y)),
        (
            Value::AnonStruct(AnonStructValue { members: m1 }),
            Value::AnonStruct(AnonStructValue { members: m2 }),
        )
        | (
            Value::Struct(StructValue {
                anon_struct: AnonStructValue { members: m1 },
                ..
            }),
            Value::Struct(StructValue {
                anon_struct: AnonStructValue { members: m2 },
                ..
            }),
        ) => members_eq(m1, m2),
        _ => false,
    }
}

fn members_eq(m1: &FxHashMap<String, Value>, m2: &FxHashMap<String, Value>) -> bool {
    m1.len() == m2.len()
        && m1
            .iter()
            .all(|(name, v1)| m2.get(name).is_some_and(|v2| values_eq(v1, v2)))
}

/// The anon-struct value `{ a = U32(0), b = String("") }`.
fn anon_struct_val() -> Value {
    let mut m: FxHashMap<String, Value> = FxHashMap::default();
    m.insert("a".to_string(), v_u32(0));
    m.insert("b".to_string(), v_string(""));
    Value::AnonStruct(AnonStructValue { members: m })
}

/// The array value `[ I32(0), I32(0), I32(0) ]: A`.
fn array_val() -> Value {
    Value::Array(ArrayValue {
        anon_array: AnonArrayValue {
            elements: vec![v_i32(0), v_i32(0), v_i32(0)],
        },
        ty: default_array(),
    })
}

/// The struct value `{ a = U32(0), b = String("") }: S`.
fn struct_val() -> Value {
    let mut m: FxHashMap<String, Value> = FxHashMap::default();
    m.insert("a".to_string(), v_u32(0));
    m.insert("b".to_string(), v_string(""));
    Value::Struct(StructValue {
        anon_struct: AnonStructValue { members: m },
        ty: struct_ty("S", anon_struct(&[("a", u32()), ("b", string(None))]), 3),
    })
}

/// Drives a table of `(lhs, rhs, expected)` cases through a binary op.
/// `Some(v)` means `Ok` (checked with `values_eq`); `None` means `Err`.
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
            (Err(_), None) => {}
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
// "add" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_add() {
    with_test_ctx(|| {
        let cases: Vec<(Value, Value, Option<Value>)> = vec![
            (v_i8(1), v_i8(2), Some(v_i8(3))),
            (v_i8(1), v_integer(2), Some(v_integer(3))),
            (v_i8(1), v_f32(2.0), Some(v_f64(3.0))),
            (v_f32(1.0), v_f32(2.0), Some(v_f32(3.0))),
            (v_f32(1.0), v_f64(2.0), Some(v_f64(3.0))),
            (v_enum_constant("X", 0), v_i32(1), Some(v_i32(1))),
            (v_enum_constant("X", 0), v_i16(1), Some(v_integer(1))),
            (v_i32(1), v_string("abc"), None),
            (v_i32(1), v_anon_array(3, v_u32(0)), None),
            (v_i32(1), array_val(), None),
            (v_i32(1), anon_struct_val(), None),
            (v_i32(1), struct_val(), None),
            (v_string("abc"), v_string("def"), Some(v_string("abcdef"))),
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
            (v_i8(1), v_i8(2), Some(v_i8(-1))),
            (v_i8(1), v_integer(2), Some(v_integer(-1))),
            (v_i8(1), v_f32(2.0), Some(v_f64(-1.0))),
            (v_f32(1.0), v_f32(2.0), Some(v_f32(-1.0))),
            (v_f32(1.0), v_f64(2.0), Some(v_f64(-1.0))),
            (v_enum_constant("X", 0), v_i32(1), Some(v_i32(-1))),
            (v_enum_constant("X", 0), v_i16(1), Some(v_integer(-1))),
            (v_i32(1), v_string("abc"), None),
            (v_i32(1), v_anon_array(3, v_u32(0)), None),
            (v_i32(1), array_val(), None),
            (v_i32(1), anon_struct_val(), None),
            (v_i32(1), struct_val(), None),
            (v_string("abc"), v_string("def"), None),
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
            (v_i8(1), v_i8(2), Some(v_i8(2))),
            (v_i8(1), v_integer(2), Some(v_integer(2))),
            (v_i8(1), v_f32(2.0), Some(v_f64(2.0))),
            (v_f32(1.0), v_f32(2.0), Some(v_f32(2.0))),
            (v_f32(1.0), v_f64(2.0), Some(v_f64(2.0))),
            (v_enum_constant("X", 0), v_i32(1), Some(v_i32(0))),
            (v_enum_constant("X", 0), v_i16(1), Some(v_integer(0))),
            (v_i32(1), v_string("abc"), None),
            (v_i32(1), v_anon_array(3, v_u32(0)), None),
            (v_i32(1), array_val(), None),
            (v_i32(1), anon_struct_val(), None),
            (v_i32(1), struct_val(), None),
            (v_string("abc"), v_string("def"), None),
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
            (v_i8(1), v_i8(2), Some(v_i8(0))),
            (v_i8(1), v_integer(2), Some(v_integer(0))),
            (v_i8(1), v_f32(2.0), Some(v_f64(0.5))),
            (v_f32(1.0), v_f32(2.0), Some(v_f32(0.5))),
            (v_f32(1.0), v_f64(2.0), Some(v_f64(0.5))),
            (v_enum_constant("X", 0), v_i32(1), Some(v_i32(0))),
            (v_enum_constant("X", 0), v_i16(1), Some(v_integer(0))),
            (v_i32(1), v_string("abc"), None),
            (v_i32(1), v_anon_array(3, v_u32(0)), None),
            (v_i32(1), array_val(), None),
            (v_i32(1), anon_struct_val(), None),
            (v_i32(1), struct_val(), None),
            (v_string("abc"), v_string("def"), None),
        ];
        check_binop("/", |a, b| a.div(b), cases);
    });
}

// ---------------------------------------------------------------------------
// "convert to type" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_convert_to_type() {
    with_test_ctx(|| {
        // Expected anon-struct { a = U32(0), b = String(""), c = I8(0) } for the
        // widening case that adds a defaulted member.
        let abc_struct = {
            let mut m: FxHashMap<String, Value> = FxHashMap::default();
            m.insert("a".to_string(), v_u32(0));
            m.insert("b".to_string(), v_string(""));
            m.insert("c".to_string(), v_i8(0));
            Value::AnonStruct(AnonStructValue { members: m })
        };
        let cases: Vec<(Value, Arc<Type>, Option<Value>)> = vec![
            (v_i8(1), i8(), Some(v_i8(1))),
            (v_i8(1), i16(), Some(v_i16(1))),
            (v_i8(1), f32(), Some(v_f32(1.0))),
            (v_integer(1), i8(), Some(v_i8(1))),
            (v_i8(1), integer(), Some(v_integer(1))),
            (v_f64(1.0), integer(), Some(v_integer(1))),
            (v_f64(1.0), f32(), Some(v_f32(1.0))),
            (v_enum_constant("X", 0), f32(), Some(v_f32(0.0))),
            (v_i32(0), default_enum(), None),
            (
                v_i32(42),
                anon_array(Some(3), u32()),
                Some(v_anon_array(3, v_u32(42))),
            ),
            (anon_struct_val(), anon_array(Some(3), u32()), None),
            (
                array_val(),
                anon_array(Some(3), u32()),
                Some(v_anon_array(3, v_u32(0))),
            ),
            (anon_struct_val(), default_array(), None),
            (v_anon_array(2, v_i8(1)), anon_array(Some(3), u32()), None),
            (
                anon_struct_val(),
                anon_struct(&[("a", u32()), ("b", string(None))]),
                Some(anon_struct_val()),
            ),
            (anon_struct_val(), anon_struct(&[("a", string(None))]), None),
            (
                anon_struct_val(),
                anon_struct(&[("a", u32()), ("b", string(None)), ("c", i8())]),
                Some(abc_struct.clone()),
            ),
            (
                struct_val(),
                anon_struct(&[("a", u32()), ("b", string(None)), ("c", i8())]),
                Some(abc_struct),
            ),
            (
                v_i32(0),
                anon_struct(&[("a", u32()), ("b", string(None))]),
                None,
            ),
            (
                v_i32(0),
                anon_struct(&[("a", u32())]),
                Some({
                    let mut m: FxHashMap<String, Value> = FxHashMap::default();
                    m.insert("a".to_string(), v_u32(0));
                    Value::AnonStruct(AnonStructValue { members: m })
                }),
            ),
        ];
        check_convert(cases);
    });
}

// ---------------------------------------------------------------------------
// Additional coverage: conversion paths and `Display` not otherwise
// exercised by the cases above.
// ---------------------------------------------------------------------------

/// A scalar promotes to a *named* array/struct target, yielding a `Value::Array`
/// / `Value::Struct` (the promotion arms that build named aggregates).
#[test]
fn value_convert_scalar_to_named_aggregates() {
    with_test_ctx(|| {
        // Scalar -> named array `A = [2] U32`.
        let arr_ty = array("A", anon_array(Some(2), u32()), 500);
        match v_i32(7).convert(&arr_ty) {
            Some(Value::Array(a)) => {
                assert_eq!(a.anon_array.elements.len(), 2);
                assert!(
                    a.anon_array
                        .elements
                        .iter()
                        .all(|e| values_eq(e, &v_u32(7)))
                );
            }
            other => panic!("expected Value::Array, got {other:?}"),
        }

        // Scalar -> named struct `S { a: U32, b: U32 }`.
        let s_ty = struct_ty("S", anon_struct(&[("a", u32()), ("b", u32())]), 501);
        match v_i32(3).convert(&s_ty) {
            Some(Value::Struct(s)) => {
                assert_eq!(s.anon_struct.members.len(), 2);
                assert!(values_eq(
                    s.anon_struct.members.get("a").unwrap(),
                    &v_u32(3)
                ));
                assert!(values_eq(
                    s.anon_struct.members.get("b").unwrap(),
                    &v_u32(3)
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
        assert!(values_eq(&v_f64(2.0).convert(&i32()).unwrap(), &v_i32(2)));
        assert!(values_eq(&v_f64(2.0).convert(&f32()).unwrap(), &v_f32(2.0)));
        assert!(values_eq(
            &v_f64(2.0).convert(&integer()).unwrap(),
            &v_integer(2)
        ));
        assert!(v_f64(2.0).convert(&boolean()).is_none());
    });
}

/// A bool converts to `bool` and nothing else; a string converts to any string.
#[test]
fn value_convert_bool_and_string() {
    with_test_ctx(|| {
        assert!(values_eq(
            &v_bool(true).convert(&boolean()).unwrap(),
            &v_bool(true)
        ));
        assert!(v_bool(true).convert(&i32()).is_none());

        assert!(values_eq(
            &v_string("hi").convert(&string(Some(8))).unwrap(),
            &v_string("hi")
        ));
        assert!(v_string("hi").convert(&boolean()).is_none());
    });
}

/// A value whose type is a named definition converts to that same definition
/// via the identity fast path (no structural rebuild).
#[test]
fn value_convert_named_identity() {
    with_test_ctx(|| {
        // Same enum definition id -> identity conversion succeeds.
        let e = enumeration("E", fpp_ast::IntegerKind::I32, 510);
        let ec = Value::EnumConstant(EnumConstantValue::new("X".to_string(), 1, e.clone()));
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
            (v_integer(1), v_f32(2.0), Some(v_f64(3.0))),
            (v_integer(1), v_integer(2), Some(v_integer(3))),
            (v_integer(4), v_enum_constant("X", 1), Some(v_integer(5))),
            (v_integer(1), v_string("x"), None),
            // Float-left arm
            (v_f64(1.0), v_integer(2), Some(v_f64(3.0))),
            (v_f32(1.0), v_f64(2.0), Some(v_f64(3.0))),
            (v_f64(1.0), v_string("x"), None),
            // enum-left arm with matching rep type -> keeps the primitive kind
            (v_enum_constant("X", 2), v_i32(3), Some(v_i32(5))),
        ];
        check_binop("+", |a, b| a.add(b), cases);

        // Integer / 0 -> DivByZero (the i128 div-by-zero guard).
        let zero: Vec<(Value, Value, Option<Value>)> = vec![(v_integer(1), v_integer(0), None)];
        check_binop("/", |a, b| a.div(b), zero);
    });
}

/// `Display` for every `Value` variant.
#[test]
fn value_display() {
    with_test_ctx(|| {
        assert_eq!(v_i8(5).to_string(), "5");
        assert_eq!(v_u8(200).to_string(), "200");
        assert_eq!(v_integer(-3).to_string(), "-3");
        assert_eq!(v_f64(1.5).to_string(), "1.5");
        assert_eq!(v_bool(true).to_string(), "true");
        assert_eq!(v_string("hi").to_string(), "\"hi\"");
        assert_eq!(v_enum_constant("X", 7).to_string(), "7");
        assert_eq!(v_anon_array(2, v_u32(0)).to_string(), "[array value]");
        assert_eq!(array_val().to_string(), "[array value]");
        assert_eq!(anon_struct_val().to_string(), "{struct value}");
        assert_eq!(struct_val().to_string(), "{struct value}");
    });
}

// ---------------------------------------------------------------------------
// "get_type" should ...
//
// Compared with `types_structurally_eq`.
// ---------------------------------------------------------------------------

#[test]
fn value_get_type() {
    with_test_ctx(|| {
        // Every primitive int kind, plus Integer/Float/Boolean/String.
        let prim = |kind| Value::PrimitiveInteger(PrimitiveIntegerValue { value: 0, kind });
        use fpp_ast::IntegerKind::*;
        let scalar_cases: Vec<(Value, Arc<Type>)> = vec![
            (prim(I8), i8()),
            (prim(I16), i16()),
            (prim(I32), i32()),
            (prim(I64), i64()),
            (prim(U8), u8()),
            (prim(U16), u16()),
            (prim(U32), u32()),
            (prim(U64), u64()),
            (v_integer(0), integer()),
            (v_f32(0.0), f32()),
            (v_f64(0.0), f64()),
            (v_bool(false), boolean()),
            (v_string(""), string(None)),
        ];
        for (v, expected) in scalar_cases {
            assert!(
                types_structurally_eq(&v.get_type(), &expected),
                "get_type({v}) expected `{expected}`, got `{}`",
                v.get_type()
            );
        }

        // AnonArray value -> AnonArray(Some(3), U32)
        let aa = v_anon_array(3, v_u32(0));
        assert!(types_structurally_eq(
            &aa.get_type(),
            &anon_array(Some(3), u32())
        ));

        // Array value -> its named array type A (= default_array()).
        assert!(types_structurally_eq(
            &array_val().get_type(),
            &default_array()
        ));

        // Enum constant -> its named enum type (default_enum()).
        let ec = v_enum_constant("X", 0);
        assert!(types_structurally_eq(&ec.get_type(), &default_enum()));

        // AnonStruct value -> AnonStruct { a: U32, b: string }.
        let as_ty = anon_struct(&[("a", u32()), ("b", string(None))]);
        assert!(types_structurally_eq(&anon_struct_val().get_type(), &as_ty));

        // Struct value -> its named struct type S.
        let s_ty = struct_ty("S", anon_struct(&[("a", u32()), ("b", string(None))]), 3);
        assert!(types_structurally_eq(&struct_val().get_type(), &s_ty));
    });
}

// ---------------------------------------------------------------------------
// "lshift" / "rshift" should ...
//
// `Some(v)` => `Some` (compared with `values_eq`); `None` => `None`.
// ---------------------------------------------------------------------------

#[track_caller]
fn check_shift(
    op_name: &str,
    op: impl Fn(&Value, &Value) -> Option<Value>,
    cases: Vec<(Value, Value, Option<Value>)>,
) {
    for (a, b, expected) in cases {
        let got = op(&a, &b);
        match (&got, &expected) {
            (Some(g), Some(e)) => {
                assert!(values_eq(g, e), "{a} {op_name} {b}: expected {e}, got {g}")
            }
            (None, None) => {}
            (Some(g), None) => panic!("{a} {op_name} {b}: expected None, got {g}"),
            (None, Some(e)) => panic!("{a} {op_name} {b}: expected {e}, got None"),
        }
    }
}

#[test]
fn value_lshift() {
    with_test_ctx(|| {
        let cases: Vec<(Value, Value, Option<Value>)> = vec![
            (v_i8(1), v_i8(2), Some(v_i8(4))),
            (v_i32(1), v_i32(3), Some(v_i32(8))),
            (v_i8(-1), v_i8(1), Some(v_i8(-2))),
            (v_i32(1), v_integer(4), Some(v_i32(16))),
            (v_integer(1), v_i32(5), Some(v_integer(32))),
            (v_integer(1), v_integer(10), Some(v_integer(1024))),
            (v_enum_constant("X", 0), v_i32(3), Some(v_i32(0))),
            (v_enum_constant("X", 0), v_integer(3), Some(v_i32(0))),
            (v_i32(1), v_enum_constant("X", 0), Some(v_i32(1))),
            (v_integer(1), v_enum_constant("X", 0), Some(v_integer(1))),
            (v_i32(1), v_f32(2.0), None),
            (v_f32(1.0), v_i32(2), None),
            (v_i32(1), v_string(""), None),
            (v_string(""), v_i32(2), None),
            (v_i32(1), v_bool(false), None),
            (v_bool(false), v_i32(2), None),
            (v_i32(1), v_anon_array(3, v_u32(0)), None),
            (v_i32(1), array_val(), None),
            (v_i32(1), anon_struct_val(), None),
            (v_i32(1), struct_val(), None),
        ];
        check_shift("<<", |a, b| a.shl(b), cases);
    });
}

#[test]
fn value_rshift() {
    with_test_ctx(|| {
        let cases: Vec<(Value, Value, Option<Value>)> = vec![
            (v_i8(8), v_i8(1), Some(v_i8(4))),
            (v_i32(16), v_i32(2), Some(v_i32(4))),
            (v_i8(-8), v_i8(1), Some(v_i8(-4))),
            (v_i32(16), v_integer(2), Some(v_i32(4))),
            (v_integer(32), v_i32(2), Some(v_integer(8))),
            (v_integer(64), v_integer(3), Some(v_integer(8))),
            (v_enum_constant("X", 0), v_i32(1), Some(v_i32(0))),
            (v_enum_constant("X", 0), v_integer(1), Some(v_i32(0))),
            (v_i32(16), v_enum_constant("X", 0), Some(v_i32(16))),
            (v_integer(16), v_enum_constant("X", 0), Some(v_integer(16))),
            (v_i32(16), v_f32(2.0), None),
            (v_f32(16.0), v_i32(2), None),
            (v_i32(16), v_string(""), None),
            (v_string(""), v_i32(2), None),
            (v_i32(16), v_bool(false), None),
            (v_bool(false), v_i32(2), None),
            (v_i32(16), v_anon_array(3, v_u32(0)), None),
            (v_i32(16), array_val(), None),
            (v_i32(16), anon_struct_val(), None),
            (v_i32(16), struct_val(), None),
        ];
        check_shift(">>", |a, b| a.shr(b), cases);
    });
}

// ---------------------------------------------------------------------------
// "is zero" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_is_zero() {
    with_test_ctx(|| {
        assert!(!v_i32(1).is_zero());
        assert!(v_i32(0).is_zero());
        // default_enum's "X" constant with value 0 is zero.
        assert!(v_enum_constant("X", 0).is_zero());
        assert!(!v_f32(1.0).is_zero());
        assert!(v_f32(0.0).is_zero());
        assert!(!array_val().is_zero());
        assert!(!v_anon_array(3, v_u32(0)).is_zero());
        assert!(!struct_val().is_zero());
        assert!(!anon_struct_val().is_zero());
    });
}

// ---------------------------------------------------------------------------
// "negate" should ...
// ---------------------------------------------------------------------------

#[test]
fn value_negate() {
    with_test_ctx(|| {
        assert!(values_eq(&v_i8(1).negate().unwrap(), &v_i8(-1)));
        assert!(values_eq(&v_f32(1.0).negate().unwrap(), &v_f32(-1.0)));
        assert!(values_eq(&v_integer(1).negate().unwrap(), &v_integer(-1)));
        assert!(v_string("").negate().is_none());
        assert!(v_anon_array(3, v_u32(0)).negate().is_none());
        assert!(anon_struct_val().negate().is_none());
        // Enum constant negates through its rep type (default_enum rep is I32).
        assert!(values_eq(
            &v_enum_constant("X", 1).negate().unwrap(),
            &v_i32(-1)
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
        assert!(values_eq(&v_u8(256).truncate(), &v_u8(0)));
        assert!(values_eq(&v_i8(256).truncate(), &v_i8(0)));
        assert!(values_eq(&v_u8(257).truncate(), &v_u8(1)));
        assert!(values_eq(&v_i8(257).truncate(), &v_i8(1)));
        assert!(values_eq(&v_u8(-1).truncate(), &v_u8(255)));

        // Array truncation is elementwise.
        assert!(values_eq(
            &v_anon_array(3, v_u8(256)).truncate(),
            &v_anon_array(3, v_u8(0))
        ));
        assert!(values_eq(
            &v_anon_array(3, v_i8(256)).truncate(),
            &v_anon_array(3, v_i8(0))
        ));
        assert!(values_eq(
            &v_anon_array(3, v_u8(257)).truncate(),
            &v_anon_array(3, v_u8(1))
        ));
        assert!(values_eq(
            &v_anon_array(3, v_i8(257)).truncate(),
            &v_anon_array(3, v_i8(1))
        ));
        assert!(values_eq(
            &v_anon_array(3, v_u8(-1)).truncate(),
            &v_anon_array(3, v_u8(255))
        ));
    });
}
