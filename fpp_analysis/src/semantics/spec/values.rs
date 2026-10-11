//! Helper functions and values for values

use super::types::{
    anon_array, anon_struct, as_array_ty, as_enum_ty, as_struct_ty, default_array, default_enum,
    string, struct_, u32,
};
use crate::semantics::{
    AnonArrayValue, AnonStructValue, ArrayValue, BooleanValue, EnumConstantValue, FloatValue,
    IntegerValue, PrimitiveIntegerValue, StringValue, StructValue, Type, Value,
};
use fpp_ast::{FloatKind, IntegerKind};
use indexmap::IndexMap;
use std::sync::Arc;

/// A primitive integer value of kind `kind`
fn create_primitive_int(v: i128, kind: IntegerKind) -> Value {
    Value::PrimitiveInteger(PrimitiveIntegerValue { value: v, kind })
}

pub fn create_i8(v: i128) -> Value {
    create_primitive_int(v, IntegerKind::I8)
}
pub fn create_i16(v: i128) -> Value {
    create_primitive_int(v, IntegerKind::I16)
}
pub fn create_i32(v: i128) -> Value {
    create_primitive_int(v, IntegerKind::I32)
}
pub fn create_i64(v: i128) -> Value {
    create_primitive_int(v, IntegerKind::I64)
}
pub fn create_u8(v: i128) -> Value {
    create_primitive_int(v, IntegerKind::U8)
}
pub fn create_u16(v: i128) -> Value {
    create_primitive_int(v, IntegerKind::U16)
}
pub fn create_u32(v: i128) -> Value {
    create_primitive_int(v, IntegerKind::U32)
}
pub fn create_u64(v: i128) -> Value {
    create_primitive_int(v, IntegerKind::U64)
}

pub fn default_i8() -> Value {
    create_i8(0)
}
pub fn default_i16() -> Value {
    create_i16(0)
}
pub fn default_i32() -> Value {
    create_i32(0)
}
pub fn default_i64() -> Value {
    create_i64(0)
}
pub fn default_u8() -> Value {
    create_u8(0)
}
pub fn default_u16() -> Value {
    create_u16(0)
}
pub fn default_u32() -> Value {
    create_u32(0)
}
pub fn default_u64() -> Value {
    create_u64(0)
}

pub fn default_integer() -> Value {
    Value::Integer(IntegerValue(0))
}

pub fn create_f32(v: f64) -> Value {
    Value::Float(FloatValue {
        value: v,
        kind: FloatKind::F32,
    })
}
pub fn create_f64(v: f64) -> Value {
    Value::Float(FloatValue {
        value: v,
        kind: FloatKind::F64,
    })
}

pub fn default_f32() -> Value {
    create_f32(0.0)
}
pub fn default_f64() -> Value {
    create_f64(0.0)
}

pub fn default_boolean() -> Value {
    Value::Boolean(BooleanValue(false))
}

pub fn default_string() -> Value {
    Value::String(StringValue("".to_string()))
}

/// An anonymous array value of `size` copies of `v`, sharing the one element
pub fn create_anon_array(size: usize, v: Value) -> Value {
    Value::AnonArray(AnonArrayValue::repeated(v, size))
}
pub fn default_anon_array_3_u32() -> Value {
    create_anon_array(3, default_u32())
}
pub fn default_anon_array_3_u32_type() -> Arc<Type> {
    anon_array(Some(3), u32())
}
pub fn default_anon_array_3_i32() -> AnonArrayValue {
    AnonArrayValue::new(vec![default_i32(), default_i32(), default_i32()])
}
/// The array value `[ 0: I32, 0: I32, 0: I32 ]: A`
pub fn array_value() -> Value {
    Value::Array(ArrayValue {
        anon_array: default_anon_array_3_i32(),
        ty: as_array_ty(&array_type()),
    })
}
pub fn array_type() -> Arc<Type> {
    default_array()
}

/// The enum constant value `(X,0): E`
pub fn enumeration_value() -> Value {
    Value::EnumConstant(EnumConstantValue::new(
        "X".to_string(),
        0,
        as_enum_ty(&enum_type()),
    ))
}
pub fn enum_type() -> Arc<Type> {
    default_enum()
}

/// The anonymous struct value `{ a = 0: U32, b = "" }`
pub fn anon_struct_value() -> Value {
    Value::AnonStruct(AnonStructValue {
        members: IndexMap::from([
            ("a".to_string(), default_u32()),
            ("b".to_string(), default_string()),
        ]),
    })
}
pub fn anon_struct_type() -> Arc<Type> {
    anon_struct(&[("a", u32()), ("b", string(None))])
}
pub fn struct_type() -> Arc<Type> {
    struct_("S", anon_struct_type(), 3, &[])
}
/// The struct value `{ a = 0: U32, b = "" }: S`
pub fn struct_value() -> Value {
    let Value::AnonStruct(anon_struct) = anon_struct_value() else {
        unreachable!("anon_struct_value is an anonymous struct value")
    };
    Value::Struct(StructValue {
        anon_struct,
        ty: as_struct_ty(&struct_type()),
    })
}

// Rust-only value builders: the Scala specs construct these values directly

/// An `Integer` value
pub fn create_integer(v: i128) -> Value {
    Value::Integer(IntegerValue(v))
}

/// A Boolean value
pub fn create_boolean(v: bool) -> Value {
    Value::Boolean(BooleanValue(v))
}

/// A string value
pub fn create_string(s: &str) -> Value {
    Value::String(StringValue(s.to_string()))
}

/// An enum constant value of `default_enum()`
pub fn create_enum_constant(member: &str, value: i128) -> Value {
    Value::EnumConstant(EnumConstantValue::new(
        member.to_string(),
        value,
        as_enum_ty(&default_enum()),
    ))
}
