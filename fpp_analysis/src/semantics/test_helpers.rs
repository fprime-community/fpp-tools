#![cfg(test)]

use crate::semantics::{AnonArrayType, AnonStructType, ArrayType, EnumType, StructType, Type};
use crate::semantics::{
    AnonArrayValue, BooleanValue, EnumConstantValue, FloatValue, IntegerValue,
    PrimitiveIntegerValue, StringValue, Value,
};
use fpp_ast::{
    DefAbsType, DefAliasType, DefArray, DefEnum, DefStruct, Expr, ExprKind, FloatKind, IntegerKind,
    Name, TypeName, TypeNameKind,
};
use rustc_hash::FxHashMap as HashMap;
use std::cell::RefCell;
use std::ops::Deref;
use std::sync::Arc;

thread_local! {
    /// Memoizes `id -> Node` so that repeated builder calls with the same integer
    /// id return the same node, giving each id a stable node identity.
    static NODE_BY_ID: RefCell<HashMap<u64, fpp_core::Node>> = RefCell::new(HashMap::default());
    /// A single shared source file / span for all synthesized nodes.
    static TEST_SPAN: RefCell<Option<fpp_core::Span>> = const { RefCell::new(None) };
}

/// Runs `f` inside a fresh compiler context with the node-id memo reset.
///
/// All type/value builders below must be called inside this closure.
pub fn with_test_ctx<T>(f: impl FnOnce() -> T) -> T {
    let mut buf = vec![];
    let mut ctx = fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut buf));
    fpp_core::run(&mut ctx, || {
        NODE_BY_ID.with(|m| m.borrow_mut().clear());
        let src = fpp_core::SourceFile::new("test", "test".to_string());
        let span = fpp_core::Span::new(src, 0, 1, None);
        TEST_SPAN.with(|s| *s.borrow_mut() = Some(span));
        f()
    })
}

/// A synthesized span shared by all test nodes.
fn test_span() -> fpp_core::Span {
    TEST_SPAN.with(|s| s.borrow().expect("with_test_ctx not active"))
}

/// Returns the memoized node for `id`, creating it on first use. Same `id`
/// always yields the same [`fpp_core::Node`].
fn node_for(id: u64) -> fpp_core::Node {
    NODE_BY_ID.with(|m| {
        *m.borrow_mut()
            .entry(id)
            .or_insert_with(|| fpp_core::Node::new(test_span()))
    })
}

/// A name whose `node_id` is memoized by `id`.
fn name_with_id(data: &str, id: u64) -> Name {
    Name {
        data: data.to_string(),
        node_id: node_for(id),
    }
}

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

/// A literal-int expression node (used for string/array sizes), memoized by `id`.
fn expr_lit_int(value: &str, id: u64) -> Expr {
    Expr {
        kind: ExprKind::LiteralInt(value.to_string()),
        node_id: node_for(id),
    }
}

/// A `string size <size>` type. The size is carried as a resolved `i128`
/// (matching Rust's `Type::String(Option<i128>)`).
pub fn string_with_size(size: i128) -> Arc<Type> {
    Arc::new(Type::String(Some(size)))
}

// ---------------------------------------------------------------------------
// Named-type builders
// ---------------------------------------------------------------------------

pub fn abs_type(name: &str, id: u64) -> Arc<Type> {
    let node = DefAbsType {
        name: name_with_id(name, id),
        node_id: node_for(id),
    };
    Arc::new(Type::AbsType(crate::semantics::AbsType {
        node,
        default_value: None,
    }))
}

/// A dummy `TypeName` (`Integer(U32)`) used as a placeholder.
fn dummy_type_name(id: u64) -> TypeName {
    TypeName {
        kind: TypeNameKind::Integer(IntegerKind::U32),
        node_id: node_for(id.wrapping_add(1_000_000)),
    }
}

pub fn alias_type(name: &str, ty: Arc<Type>, id: u64) -> Arc<Type> {
    let node = DefAliasType {
        name: name_with_id(name, id),
        type_name: dummy_type_name(id),
        is_dictionary_def: false,
        node_id: node_for(id),
    };
    Arc::new(Type::AliasType(crate::semantics::AliasType {
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
    let node = DefArray {
        name: name_with_id(name, id),
        size: size_expr,
        elt_type: elt_type_name,
        default: None,
        format: None,
        is_dictionary_def: false,
        node_id: node_for(id),
    };
    Arc::new(Type::Array(ArrayType {
        node,
        anon_array: anon,
        default: None,
        format: None,
    }))
}

pub fn enumeration(name: &str, rep_type: IntegerKind, id: u64) -> Arc<Type> {
    let node = DefEnum {
        name: name_with_id(name, id),
        type_name: None,
        constants: vec![],
        default: None,
        is_dictionary_def: false,
        node_id: node_for(id),
    };
    Arc::new(Type::Enum(EnumType {
        node,
        rep_type,
        default: None,
    }))
}

pub fn struct_ty(name: &str, anon_struct: Arc<Type>, id: u64) -> Arc<Type> {
    struct_ty_sized(name, anon_struct, id, &[])
}

/// `struct` with explicit member array sizes.
pub fn struct_ty_sized(
    name: &str,
    anon_struct: Arc<Type>,
    id: u64,
    sizes: &[(&str, u32)],
) -> Arc<Type> {
    let anon = match anon_struct.deref() {
        Type::AnonStruct(s) => s.clone(),
        other => panic!("struct_ty() expects an AnonStruct, got {other:?}"),
    };
    let node = DefStruct {
        name: name_with_id(name, id),
        members: vec![],
        default: None,
        is_dictionary_def: false,
        node_id: node_for(id),
    };
    let mut size_map = HashMap::default();
    for (n, s) in sizes {
        size_map.insert((*n).to_string(), *s);
    }
    Arc::new(Type::Struct(StructType {
        node,
        anon_struct: anon,
        default: None,
        sizes: size_map,
        formats: HashMap::default(),
    }))
}

pub fn anon_array(size: Option<usize>, elt_type: Arc<Type>) -> Arc<Type> {
    Arc::new(Type::AnonArray(AnonArrayType { size, elt_type }))
}

pub fn anon_struct(members: &[(&str, Arc<Type>)]) -> Arc<Type> {
    let mut m = HashMap::default();
    for (name, ty) in members {
        m.insert((*name).to_string(), ty.clone());
    }
    Arc::new(Type::AnonStruct(AnonStructType { members: m }))
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
    struct_ty("S", anon_struct(&[]), 3)
}
pub fn default_alias_type() -> Arc<Type> {
    alias_type("TAlias", default_abs_type(), 4)
}

// ---------------------------------------------------------------------------
// Structural type equality (for asserting common-type / conversion results)
// ---------------------------------------------------------------------------

/// Structural equality on `Type`. Named types compare by definition node id;
/// anonymous aggregates compare structurally on members/elements.
pub fn types_structurally_eq(a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::PrimitiveInt(k1), Type::PrimitiveInt(k2)) => k1 == k2,
        (Type::Float(k1), Type::Float(k2)) => k1 == k2,
        (Type::Boolean, Type::Boolean) => true,
        (Type::Integer, Type::Integer) => true,
        // Sizes are resolved i128 options, so compare those directly.
        (Type::String(s1), Type::String(s2)) => s1 == s2,
        (Type::AnonArray(a1), Type::AnonArray(a2)) => {
            a1.size == a2.size && types_structurally_eq(&a1.elt_type, &a2.elt_type)
        }
        (Type::AnonStruct(s1), Type::AnonStruct(s2)) => {
            s1.members.len() == s2.members.len()
                && s1.members.iter().all(|(name, ty1)| {
                    s2.members
                        .get(name)
                        .is_some_and(|ty2| types_structurally_eq(ty1, ty2))
                })
        }
        // Named types (AbsType, AliasType, Array, Enum, Struct) compare by
        // definition node id — this is exactly `Type::identical`'s named-type arm.
        _ => match (a.def_node_id(), b.def_node_id()) {
            (Some(n1), Some(n2)) => {
                n1 == n2 && std::mem::discriminant(a) == std::mem::discriminant(b)
            }
            _ => false,
        },
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

// ---------------------------------------------------------------------------
// Value builders
// ---------------------------------------------------------------------------

pub fn v_i8(v: i128) -> Value {
    Value::PrimitiveInteger(PrimitiveIntegerValue {
        value: v,
        kind: IntegerKind::I8,
    })
}
pub fn v_i16(v: i128) -> Value {
    Value::PrimitiveInteger(PrimitiveIntegerValue {
        value: v,
        kind: IntegerKind::I16,
    })
}
pub fn v_i32(v: i128) -> Value {
    Value::PrimitiveInteger(PrimitiveIntegerValue {
        value: v,
        kind: IntegerKind::I32,
    })
}
pub fn v_u8(v: i128) -> Value {
    Value::PrimitiveInteger(PrimitiveIntegerValue {
        value: v,
        kind: IntegerKind::U8,
    })
}
pub fn v_u32(v: i128) -> Value {
    Value::PrimitiveInteger(PrimitiveIntegerValue {
        value: v,
        kind: IntegerKind::U32,
    })
}
pub fn v_integer(v: i128) -> Value {
    Value::Integer(IntegerValue(v))
}
pub fn v_f32(v: f64) -> Value {
    Value::Float(FloatValue {
        value: v,
        kind: FloatKind::F32,
    })
}
pub fn v_f64(v: f64) -> Value {
    Value::Float(FloatValue {
        value: v,
        kind: FloatKind::F64,
    })
}
pub fn v_bool(v: bool) -> Value {
    Value::Boolean(BooleanValue(v))
}
pub fn v_string(s: &str) -> Value {
    Value::String(StringValue(s.to_string()))
}

/// An enum constant value over `default_enum()`.
pub fn v_enum_constant(member: &str, value: i128) -> Value {
    Value::EnumConstant(EnumConstantValue::new(
        member.to_string(),
        value,
        default_enum(),
    ))
}

/// An anonymous array value of `size` copies of `v`.
pub fn v_anon_array(size: usize, v: Value) -> Value {
    Value::AnonArray(AnonArrayValue {
        elements: std::iter::repeat_n(v, size).collect(),
    })
}

// ---------------------------------------------------------------------------
// SerializedSize test analysis
// ---------------------------------------------------------------------------

/// Builds an [`crate::Analysis`] populated with the framework definitions that
/// `Type::serialized_size` needs: `FwSizeStoreType = U16` (the string length
/// prefix type, 2 bytes) and `FW_FIXED_LENGTH_STRING_SIZE = 256` (the default
/// string data size).
///
/// Note: string sizes are pre-resolved as `i128`, so no per-string
/// `value_map` entries are needed — only the two framework definitions.
pub fn sizeof_test_analysis() -> crate::Analysis {
    use crate::semantics::Symbol;
    use fpp_ast::DefConstant;

    let mut a = crate::Analysis::new();

    // FwSizeStoreType = U16 (id chosen high to avoid colliding with type ids).
    let store_id = 900_001;
    let store_ty = alias_type("FwSizeStoreType", u16(), store_id);
    let store_def = match store_ty.deref() {
        Type::AliasType(al) => al.node.clone(),
        _ => unreachable!(),
    };
    a.framework_definitions.type_map.insert(
        "FwSizeStoreType".to_string(),
        Symbol::AliasType(Arc::new(store_def)),
    );
    a.type_map.insert(node_for(store_id), store_ty);

    // FW_FIXED_LENGTH_STRING_SIZE = 256
    let const_id = 900_002;
    let const_def = DefConstant {
        name: name_with_id("FW_FIXED_LENGTH_STRING_SIZE", const_id),
        value: expr_lit_int("256", const_id.wrapping_add(1)),
        is_dictionary_def: false,
        node_id: node_for(const_id),
    };
    a.framework_definitions.constant_map.insert(
        "FW_FIXED_LENGTH_STRING_SIZE".to_string(),
        Symbol::Constant(Arc::new(const_def)),
    );
    a.value_map
        .insert(node_for(const_id), Value::Integer(IntegerValue(256)));

    a
}
