//! Helper functions and values for types

use super::helpers::{expr_lit_int, name_with_id, node_for};
use crate::semantics::{
    AbsType, AliasType, AnonArrayType, AnonStructType, ArrayType, EnumType, StructType, Type,
};
use fpp_ast::{
    DefAbsType, DefAliasType, DefArray, DefEnum, DefStruct, FloatKind, IntegerKind, TypeName,
    TypeNameKind,
};
use rustc_hash::FxHashMap as HashMap;
use std::ops::Deref;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Primitive type constants
// ---------------------------------------------------------------------------

pub fn i8() -> Arc<Type> {
    Arc::new(Type::PrimitiveInt(IntegerKind::I8))
}
pub fn i16() -> Arc<Type> {
    Arc::new(Type::PrimitiveInt(IntegerKind::I16))
}
pub fn i32() -> Arc<Type> {
    Arc::new(Type::PrimitiveInt(IntegerKind::I32))
}
pub fn i64() -> Arc<Type> {
    Arc::new(Type::PrimitiveInt(IntegerKind::I64))
}
pub fn u8() -> Arc<Type> {
    Arc::new(Type::PrimitiveInt(IntegerKind::U8))
}
pub fn u16() -> Arc<Type> {
    Arc::new(Type::PrimitiveInt(IntegerKind::U16))
}
pub fn u32() -> Arc<Type> {
    Arc::new(Type::PrimitiveInt(IntegerKind::U32))
}
pub fn u64() -> Arc<Type> {
    Arc::new(Type::PrimitiveInt(IntegerKind::U64))
}
pub fn f32() -> Arc<Type> {
    Arc::new(Type::Float(FloatKind::F32))
}
pub fn f64() -> Arc<Type> {
    Arc::new(Type::Float(FloatKind::F64))
}
pub fn boolean() -> Arc<Type> {
    Arc::new(Type::Boolean)
}
pub fn integer() -> Arc<Type> {
    Arc::new(Type::Integer)
}
pub fn string(size: Option<i128>) -> Arc<Type> {
    Arc::new(Type::String(size))
}

// ---------------------------------------------------------------------------
// Type builders
// ---------------------------------------------------------------------------

pub fn abs_type(name: &str, id: u64) -> Arc<Type> {
    let node = Arc::new(DefAbsType {
        name: name_with_id(name, id),
        node_id: node_for(id),
    });
    Arc::new(Type::Abs(Arc::new(AbsType { node })))
}

/// A dummy `TypeName` (`Integer(U32)`) used as a placeholder.
fn dummy_type_name(id: u64) -> TypeName {
    TypeName {
        kind: TypeNameKind::Integer(IntegerKind::U32),
        node_id: node_for(id.wrapping_add(1_000_000)),
    }
}

pub fn alias_type(name: &str, ty: Arc<Type>, id: u64) -> Arc<Type> {
    let node = Arc::new(DefAliasType {
        name: name_with_id(name, id),
        type_name: dummy_type_name(id),
        is_dictionary_def: false,
        node_id: node_for(id),
    });
    Arc::new(Type::Alias(AliasType {
        node,
        alias_type: ty,
    }))
}

pub fn array(name: &str, anon_array: Arc<Type>, id: u64) -> Arc<Type> {
    let anon = match anon_array.deref() {
        Type::AnonArray(a) => a.clone(),
        other => panic!("array() expects an AnonArray, got {other:?}"),
    };
    let size_expr = expr_lit_int("1", id.wrapping_add(2_000_000));
    let elt_type_name = dummy_type_name(id.wrapping_add(3_000_000));
    let node = Arc::new(DefArray {
        name: name_with_id(name, id),
        size: size_expr,
        elt_type: elt_type_name,
        default: None,
        format: None,
        is_dictionary_def: false,
        node_id: node_for(id),
    });
    Arc::new(Type::Array(Arc::new(ArrayType {
        node,
        anon_array: anon,
        default: None,
        format: None,
    })))
}

pub fn enumeration(name: &str, rep_type: IntegerKind, id: u64) -> Arc<Type> {
    let node = Arc::new(DefEnum {
        name: name_with_id(name, id),
        type_name: None,
        constants: vec![],
        default: None,
        is_dictionary_def: false,
        node_id: node_for(id),
    });
    Arc::new(Type::Enum(Arc::new(EnumType {
        node,
        rep_type,
        default: None,
    })))
}

/// A `string size <size>` type. A string type holds its size as the value of
/// the size expression, so no expression node is needed.
pub fn string_with_size(size: i128) -> Arc<Type> {
    Arc::new(Type::String(Some(size)))
}

/// A named struct type, with the member array sizes `sizes`
pub fn struct_(name: &str, anon_struct: Arc<Type>, id: u64, sizes: &[(&str, u32)]) -> Arc<Type> {
    let anon = match anon_struct.deref() {
        Type::AnonStruct(s) => s.clone(),
        other => panic!("struct_() expects an AnonStruct, got {other:?}"),
    };
    let node = Arc::new(DefStruct {
        name: name_with_id(name, id),
        members: vec![],
        default: None,
        is_dictionary_def: false,
        node_id: node_for(id),
    });
    let sizes = sizes
        .iter()
        .map(|(n, s)| ((*n).to_string(), *s))
        .collect::<HashMap<_, _>>();
    Arc::new(Type::Struct(Arc::new(StructType {
        node,
        anon_struct: anon,
        default: None,
        sizes,
        formats: HashMap::default(),
    })))
}

pub fn anon_array(size: Option<usize>, elt_type: Arc<Type>) -> Arc<Type> {
    Arc::new(Type::AnonArray(AnonArrayType { size, elt_type }))
}

pub fn anon_struct(members: &[(&str, Arc<Type>)]) -> Arc<Type> {
    let members = members
        .iter()
        .map(|(name, ty)| ((*name).to_string(), ty.clone()))
        .collect();
    Arc::new(Type::AnonStruct(AnonStructType { members }))
}

/// The named array type inside `ty`, as a value that names it holds it.
pub fn as_array_ty(ty: &Arc<Type>) -> Arc<ArrayType> {
    match ty.deref() {
        Type::Array(array_ty) => array_ty.clone(),
        other => panic!("expected an array type, got {other:?}"),
    }
}

/// The enum type inside `ty`, as an enum-constant value holds it.
pub fn as_enum_ty(ty: &Arc<Type>) -> Arc<EnumType> {
    match ty.deref() {
        Type::Enum(enum_ty) => enum_ty.clone(),
        other => panic!("expected an enum type, got {other:?}"),
    }
}

/// The named struct type inside `ty`, as a value that names it holds it.
pub fn as_struct_ty(ty: &Arc<Type>) -> Arc<StructType> {
    match ty.deref() {
        Type::Struct(struct_ty) => struct_ty.clone(),
        other => panic!("expected a struct type, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Default named types
// ---------------------------------------------------------------------------

pub fn default_abs_type() -> Arc<Type> {
    abs_type("T", 0)
}
pub fn default_array() -> Arc<Type> {
    array("A", anon_array(None, i32()), 1)
}
pub fn default_enum() -> Arc<Type> {
    enumeration("E", IntegerKind::I32, 2)
}
pub fn default_struct() -> Arc<Type> {
    struct_("S", anon_struct(&[]), 3, &[])
}
pub fn default_alias_type() -> Arc<Type> {
    alias_type("TAlias", default_abs_type(), 4)
}

// ---------------------------------------------------------------------------
// Structural type equality (for asserting common-type / conversion results)
// ---------------------------------------------------------------------------

/// Structural equality on `Type`: Scala's case-class equality. A named type
/// compares by its definition and by the structure it holds; anonymous
/// aggregates compare on their members or elements.
pub fn types_structurally_eq(a: &Type, b: &Type) -> bool {
    fn anon_arrays_eq(a1: &AnonArrayType, a2: &AnonArrayType) -> bool {
        a1.size == a2.size && types_structurally_eq(&a1.elt_type, &a2.elt_type)
    }
    fn anon_structs_eq(s1: &AnonStructType, s2: &AnonStructType) -> bool {
        s1.members.len() == s2.members.len()
            && s1.members.iter().all(|(name, ty1)| {
                s2.get_member(name)
                    .is_some_and(|ty2| types_structurally_eq(ty1, ty2))
            })
    }
    match (a, b) {
        (Type::PrimitiveInt(k1), Type::PrimitiveInt(k2)) => k1 == k2,
        (Type::Float(k1), Type::Float(k2)) => k1 == k2,
        (Type::Boolean, Type::Boolean) => true,
        (Type::Integer, Type::Integer) => true,
        // Sizes are resolved i128 options, so compare those directly.
        (Type::String(s1), Type::String(s2)) => s1 == s2,
        (Type::AnonArray(a1), Type::AnonArray(a2)) => anon_arrays_eq(a1, a2),
        (Type::AnonStruct(s1), Type::AnonStruct(s2)) => anon_structs_eq(s1, s2),
        (Type::Abs(t1), Type::Abs(t2)) => t1.node.node_id == t2.node.node_id,
        (Type::Alias(t1), Type::Alias(t2)) => {
            t1.node.node_id == t2.node.node_id
                && types_structurally_eq(&t1.alias_type, &t2.alias_type)
        }
        (Type::Array(t1), Type::Array(t2)) => {
            t1.node.node_id == t2.node.node_id && anon_arrays_eq(&t1.anon_array, &t2.anon_array)
        }
        (Type::Enum(t1), Type::Enum(t2)) => {
            t1.node.node_id == t2.node.node_id && t1.rep_type == t2.rep_type
        }
        (Type::Struct(t1), Type::Struct(t2)) => {
            t1.node.node_id == t2.node.node_id
                && anon_structs_eq(&t1.anon_struct, &t2.anon_struct)
                && t1.sizes == t2.sizes
        }
        _ => false,
    }
}

/// Asserts that an `Option<Arc<Type>>` common-type result equals an expected type.
#[track_caller]
pub fn assert_common_type_eq(actual: &Option<Arc<Type>>, expected: &Arc<Type>) {
    match actual {
        Some(got) => assert!(
            types_structurally_eq(got, expected),
            "expected common type `{expected}`, got `{got}`"
        ),
        None => panic!("expected common type `{expected}`, got None (no common type)"),
    }
}
