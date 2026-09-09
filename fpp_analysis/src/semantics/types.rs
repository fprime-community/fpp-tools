use crate::semantics::{
    AbsTypeValue, AnonArrayValue, AnonStructValue, BooleanValue, FloatValue, Format, IntegerValue,
    PrimitiveIntegerValue, StringValue, StructValue, Value,
};
use fpp_ast::{FloatKind, IntegerKind};
use fpp_core::Diagnostic;
use rustc_hash::FxHashMap as HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;

/// An FPP Type
#[derive(Debug, Clone)]
pub enum Type {
    /// Primitive integer types
    PrimitiveInt(IntegerKind),
    /// Floating-point types
    Float(FloatKind),
    /// The type of a string
    String(Option<i128>),
    /// The Boolean type
    Boolean,
    /// The type of arbitrary-width integers
    Integer,
    AbsType(AbsType),
    AliasType(AliasType),
    Array(ArrayType),
    AnonArray(AnonArrayType),
    Enum(EnumType),
    Struct(StructType),
    AnonStruct(AnonStructType),
}

impl Type {
    /// Get the underlying type
    pub fn underlying_type(ty: &Arc<Type>) -> Arc<Type> {
        match ty.deref() {
            Type::AliasType(alias) => Type::underlying_type(&alias.alias_type),
            _ => ty.clone(),
        }
    }

    /// Get the default value
    pub fn default_value(&self) -> Option<Value> {
        match self {
            Type::PrimitiveInt(kind) => Some(Value::PrimitiveInteger(PrimitiveIntegerValue {
                value: 0,
                kind: *kind,
            })),
            Type::Float(kind) => Some(Value::Float(FloatValue {
                value: 0.0,
                kind: *kind,
            })),
            Type::String(_) => Some(Value::String(StringValue("".to_string()))),
            Type::Boolean => Some(Value::Boolean(BooleanValue(false))),
            Type::Integer => Some(Value::Integer(IntegerValue(0))),
            Type::AliasType(ty) => ty.alias_type.default_value().clone(),
            Type::AbsType(ty) => Some(Value::AbsType(ty.default_value.clone()?)),
            Type::Array(array) => array.default.clone(),
            Type::AnonArray(arr) => Some(Value::AnonArray(AnonArrayValue {
                elements: std::iter::repeat_n(arr.elt_type.default_value()?, arr.size?).collect(),
            })),
            Type::Enum(ty) => ty.default.clone(),
            Type::Struct(def) => Some(Value::Struct(def.default.clone()?)),
            Type::AnonStruct(struct_) => {
                let mut members = vec![];
                for (name, ty) in &struct_.members {
                    members.push((name.clone(), ty.default_value()?))
                }

                Some(Value::AnonStruct(AnonStructValue {
                    members: HashMap::from_iter(members),
                }))
            }
        }
    }

    /// Get the array size
    pub fn array_size(&self) -> Option<usize> {
        match self {
            Type::AliasType(ty) => ty.alias_type.array_size(),
            Type::AnonArray(arr) => arr.size,
            Type::Array(arr) => arr.anon_array.size,
            _ => None,
        }
    }

    /// Get the definition node identifier, if any
    pub fn def_node_id(&self) -> Option<fpp_core::Node> {
        match self {
            Type::AbsType(ty) => Some(ty.node.node_id),
            Type::AliasType(ty) => Some(ty.node.node_id),
            Type::Array(ty) => Some(ty.node.node_id),
            Type::Enum(ty) => Some(ty.node.node_id),
            Type::Struct(ty) => Some(ty.node.node_id),
            _ => None,
        }
    }

    /// Does this type have numeric members?
    pub fn has_numeric_members(&self) -> bool {
        match self {
            Type::AliasType(ty) => ty.alias_type.has_numeric_members(),
            Type::Array(ty) => ty.anon_array.elt_type.has_numeric_members(),
            Type::AnonArray(ty) => ty.elt_type.has_numeric_members(),
            Type::Struct(ty) => ty
                .anon_struct
                .members
                .values()
                .all(|member| member.has_numeric_members()),
            Type::AnonStruct(ty) => ty
                .members
                .values()
                .all(|member| member.has_numeric_members()),
            _ => self.is_numeric(),
        }
    }

    /// Is this type convertible to a numeric type?
    pub fn is_convertible_to_numeric(&self) -> bool {
        match self {
            Type::AliasType(ty) => ty.alias_type.is_convertible_to_numeric(),
            Type::Enum(_) => true,
            _ => self.is_numeric(),
        }
    }

    /// Is this type promotable to an array type?
    pub fn is_promotable_to_array(&self) -> bool {
        match self {
            Type::AliasType(ty) => ty.alias_type.is_promotable_to_array(),
            Type::String(_) => true,
            Type::Boolean => true,
            Type::Enum(_) => true,
            _ => self.is_numeric(),
        }
    }

    /// Is this type displayable?
    pub fn is_displayable(&self) -> bool {
        match self {
            Type::PrimitiveInt(_) => true,
            Type::Float(_) => true,
            Type::String(_) => true,
            Type::Boolean => true,
            Type::Integer => false,
            Type::AbsType(_) => false,
            Type::AliasType(alias) => alias.alias_type.is_displayable(),
            // A named array is displayable iff its element type is; anonymous
            // aggregates (AnonArray/AnonStruct) inherit the base `false`.
            Type::Array(arr) => arr.anon_array.elt_type.is_displayable(),
            Type::AnonArray(_) => false,
            Type::Enum(_) => true,
            Type::Struct(ty) => ty
                .anon_struct
                .members
                .values()
                .all(|member| member.is_displayable()),
            Type::AnonStruct(_) => false,
        }
    }

    /// Is this type a float type?
    pub fn is_float(&self) -> bool {
        match self {
            Type::AliasType(ty) => ty.alias_type.is_float(),
            Type::Float(_) => true,
            _ => false,
        }
    }

    /// Best-effort serialized size, in bytes, for framework-independent types.
    ///
    /// Returns `None` for types whose serialized size depends on framework
    /// definitions (e.g. strings, which need `FwSizeStoreType` /
    /// `FW_FIXED_LENGTH_STRING_SIZE`) or on type finalization
    /// (arrays/structs) that is not available within the current pass set.
    pub fn primitive_serialized_size(&self) -> Option<i128> {
        match self {
            Type::AliasType(alias) => alias.alias_type.primitive_serialized_size(),
            Type::Boolean => Some(1),
            Type::Float(FloatKind::F32) => Some(4),
            Type::Float(FloatKind::F64) => Some(8),
            Type::PrimitiveInt(kind) => Some(match kind {
                IntegerKind::I8 | IntegerKind::U8 => 1,
                IntegerKind::I16 | IntegerKind::U16 => 2,
                IntegerKind::I32 | IntegerKind::U32 => 4,
                IntegerKind::I64 | IntegerKind::U64 => 8,
            }),
            Type::Enum(ty) => Type::PrimitiveInt(ty.rep_type).primitive_serialized_size(),
            _ => None,
        }
    }

    /// Compute the serialized size of a type. Unlike
    /// [`Type::primitive_serialized_size`], this
    /// resolves string, array, and struct sizes using the F Prime framework
    /// definitions (`FwSizeStoreType`, `FW_FIXED_LENGTH_STRING_SIZE`) recorded by
    /// `CheckFrameworkDefs`. Requires the type to be finalized (array/string
    /// sizes known); returns `None` otherwise.
    pub fn serialized_size(&self, a: &crate::Analysis) -> Option<i128> {
        use crate::semantics::SymbolInterface;
        match self {
            Type::AliasType(alias) => alias.alias_type.serialized_size(a),
            Type::Boolean => Some(1),
            Type::Float(FloatKind::F32) => Some(4),
            Type::Float(FloatKind::F64) => Some(8),
            Type::PrimitiveInt(_) => self.primitive_serialized_size(),
            Type::Enum(ty) => Type::PrimitiveInt(ty.rep_type).serialized_size(a),
            Type::Array(arr) => {
                let n = arr.anon_array.size? as i128;
                Some(n * arr.anon_array.elt_type.serialized_size(a)?)
            }
            Type::AnonArray(arr) => {
                let n = arr.size? as i128;
                Some(n * arr.elt_type.serialized_size(a)?)
            }
            Type::String(size) => {
                let store_symbol = a.framework_definitions.type_map.get("FwSizeStoreType")?;
                let store_size = a.type_map.get(&store_symbol.node())?.serialized_size(a)?;
                let data_size = match size {
                    Some(n) => *n,
                    None => {
                        let c = a
                            .framework_definitions
                            .constant_map
                            .get("FW_FIXED_LENGTH_STRING_SIZE")?;
                        a.get_int_value(c.node())?
                    }
                };
                Some(store_size + data_size)
            }
            Type::Struct(ty) => {
                let mut total = 0i128;
                for (name, member_ty) in &ty.anon_struct.members {
                    let member_size = member_ty.serialized_size(a)?;
                    let mult = ty.sizes.get(name).copied().unwrap_or(1) as i128;
                    total += member_size * mult;
                }
                Some(total)
            }
            Type::AnonStruct(ty) => {
                let mut total = 0i128;
                for member_ty in ty.members.values() {
                    total += member_ty.serialized_size(a)?;
                }
                Some(total)
            }
            Type::Integer | Type::AbsType(_) => None,
        }
    }

    /// Is this type an int type?
    pub fn is_int(&self) -> bool {
        match self {
            Type::AliasType(ty) => ty.alias_type.is_int(),
            Type::PrimitiveInt(_) => true,
            Type::Integer => true,
            _ => false,
        }
    }

    /// Is this type a primitive type?
    pub fn is_primitive(&self) -> bool {
        match self {
            Type::AliasType(ty) => ty.alias_type.is_primitive(),
            Type::PrimitiveInt(_) => true,
            Type::Float(_) => true,
            Type::Boolean => true,
            _ => false,
        }
    }

    /// Is this type a canonical (non-aliased) type?
    pub fn is_canonical(&self) -> bool {
        !matches!(self, Type::AliasType(_))
    }

    /// Is this type promotable to a struct type?
    pub fn is_promotable_to_struct(&self) -> bool {
        self.is_promotable_to_array()
    }

    /// Is this type numeric?
    pub fn is_numeric(&self) -> bool {
        match self {
            Type::AliasType(ty) => ty.alias_type.is_numeric(),
            _ => self.is_int() || self.is_float(),
        }
    }

    /// Is `from` convertible to `to`?
    pub fn convert(from: &Arc<Type>, to: &Arc<Type>) -> TypeConversionResult {
        Type::convert_impl(
            Type::underlying_type(from).deref(),
            Type::underlying_type(to).deref(),
        )
    }

    fn convert_impl(from: &Type, to: &Type) -> TypeConversionResult {
        assert!(from.is_canonical());
        assert!(to.is_canonical());

        if Self::identical(from, to) {
            return Ok(());
        }

        if from.is_convertible_to_numeric() && to.is_numeric() {
            return Ok(());
        }

        match (from, to) {
            // String -> String
            (Type::String(_), Type::String(_)) => Ok(()),

            // Array -> Array
            (
                Type::Array(ArrayType {
                    anon_array: from_arr,
                    ..
                })
                | Type::AnonArray(from_arr),
                Type::Array(ArrayType {
                    anon_array: to_arr, ..
                })
                | Type::AnonArray(to_arr),
            ) => {
                // Check the sizes match
                match (&from_arr.size, &to_arr.size) {
                    (Some(from_size), Some(to_size)) if from_size != to_size => {
                        return Err(TypeConversionError::ArraySizeMismatch {
                            from: *from_size,
                            to: *to_size,
                        });
                    }
                    _ => {}
                }

                match Type::convert(&from_arr.elt_type, &to_arr.elt_type) {
                    Ok(()) => Ok(()),
                    Err(err) => Err(TypeConversionError::ArrayElement(Box::new(err))),
                }
            }

            // Convert a single element to an array
            (
                _,
                Type::Array(ArrayType {
                    anon_array: to_arr, ..
                })
                | Type::AnonArray(to_arr),
            ) => {
                if !from.is_promotable_to_array() {
                    return Err(TypeConversionError::NotPromotableToArray(Box::new(
                        from.clone(),
                    )));
                }

                match Type::convert_impl(from, Type::underlying_type(&to_arr.elt_type).deref()) {
                    Ok(_) => Ok(()),
                    Err(err) => Err(TypeConversionError::ArrayElementDuringPromotion(Box::new(
                        err,
                    ))),
                }
            }

            // Struct -> Struct
            (
                Type::Struct(StructType {
                    anon_struct: from_struct,
                    ..
                })
                | Type::AnonStruct(from_struct),
                Type::Struct(StructType {
                    anon_struct: to_struct,
                    ..
                })
                | Type::AnonStruct(to_struct),
            ) => {
                // May sure all the members in 'from' can fit into 'to'
                for (name, from_member_ty) in &from_struct.members {
                    match to_struct.members.get(name) {
                        None => return Err(TypeConversionError::MissingStructMember(name.clone())),
                        Some(to_member_ty) => match Type::convert(from_member_ty, to_member_ty) {
                            Ok(_) => {}
                            Err(err) => {
                                return Err(TypeConversionError::StructMember {
                                    name: name.clone(),
                                    err: Box::new(err),
                                });
                            }
                        },
                    }
                }

                Ok(())
            }

            // Convert a single element to a struct
            (
                _,
                Type::Struct(StructType {
                    anon_struct: to_struct,
                    ..
                })
                | Type::AnonStruct(to_struct),
            ) => {
                if !from.is_promotable_to_struct() {
                    return Err(TypeConversionError::NotPromotableToStruct(Box::new(
                        from.clone(),
                    )));
                }

                // Make sure that 'from' can fit all members in to_struct
                for (name, to_member_ty) in &to_struct.members {
                    let to_member_ty_underlying = Type::underlying_type(to_member_ty);
                    match Type::convert_impl(from, to_member_ty_underlying.deref()) {
                        Ok(_) => {}
                        Err(err) => {
                            return Err(TypeConversionError::StructMember {
                                name: name.clone(),
                                err: Box::new(err),
                            });
                        }
                    }
                }

                Ok(())
            }

            _ => Err(TypeConversionError::Mismatch {
                from: Box::new(from.clone()),
                to: Box::new(to.clone()),
            }),
        }
    }

    /// Check for type identity
    pub fn identical(t1: &Type, t2: &Type) -> bool {
        match (t1, t2) {
            (Type::PrimitiveInt(k1), Type::PrimitiveInt(k2)) => k1 == k2,
            (Type::Float(k1), Type::Float(k2)) => k1 == k2,
            (Type::Integer, Type::Integer) => true,
            (Type::Boolean, Type::Boolean) => true,
            _ => match (t1.def_node_id(), t2.def_node_id()) {
                (Some(n1), Some(n2)) => n1 == n2,
                _ => false,
            },
        }
    }

    /// Compute the common type for a pair of types
    pub fn common_type(t1_a: &Arc<Type>, t2_a: &Arc<Type>) -> Option<Arc<Type>> {
        // Trivial case, types are the same
        if Type::identical(t1_a, t2_a) {
            return Some(t1_a.clone());
        }

        // Types share a common ancestor in the alias type hierarchy
        if !t1_a.is_canonical() || !t2_a.is_canonical() {
            fn lca(a: &Arc<Type>, b: &Arc<Type>) -> Option<Arc<Type>> {
                fn get_ancestors(t: &Arc<Type>, out: &mut Vec<Arc<Type>>) {
                    out.push(t.clone());
                    if let Type::AliasType(AliasType { alias_type, .. }) = t.deref() {
                        get_ancestors(alias_type, out)
                    }
                }

                let mut ancestors_of_a = vec![];
                get_ancestors(a, &mut ancestors_of_a);

                let mut ancestors_of_b = vec![];
                get_ancestors(b, &mut ancestors_of_b);

                // Traverse the ancestry of 'b' from youngest to oldest until we find
                // a common ancestor with 'a'. Youngest-first order yields the least
                // common ancestor.
                ancestors_of_b
                    .iter()
                    .find(|b| ancestors_of_a.iter().any(|a| Type::identical(a, b)))
                    .cloned()
            }

            if let Some(ty) = lca(t1_a, t2_a) {
                return Some(ty);
            }
        }

        // Do the rest of the operations on the underlying types since none of aliases
        // in the parent chain matched
        let t1 = Type::underlying_type(t1_a);
        let t2 = Type::underlying_type(t2_a);

        // Check for numeric common types
        if t1.is_float() && t2.is_numeric() {
            return Some(Arc::new(Type::Float(FloatKind::F64)));
        }
        if t1.is_numeric() && t2.is_float() {
            return Some(Arc::new(Type::Float(FloatKind::F64)));
        }
        if t1.is_numeric() && t2.is_numeric() {
            return Some(Arc::new(Type::Integer));
        }

        match (t1.deref(), t2.deref()) {
            // String -> String
            (Type::String(_), Type::String(_)) => Some(Arc::new(Type::String(None))),

            // Strip off any enum wrappers over the representable type
            (Type::Enum(EnumType { rep_type, .. }), _) => {
                Self::common_type(&Arc::new(Type::PrimitiveInt(*rep_type)), &t2)
            }
            (_, Type::Enum(EnumType { rep_type, .. })) => {
                Self::common_type(&t1, &Arc::new(Type::PrimitiveInt(*rep_type)))
            }

            // t1 + t2 are both array/anon array
            (
                Type::Array(ArrayType {
                    anon_array: t1_arr, ..
                })
                | Type::AnonArray(t1_arr),
                Type::Array(ArrayType {
                    anon_array: t2_arr, ..
                })
                | Type::AnonArray(t2_arr),
            ) => {
                // Check if the sizes match
                let size = match (t1_arr.size, t2_arr.size) {
                    (Some(t1_size), Some(t2_size)) => {
                        if t1_size == t2_size {
                            Some(t1_size)
                        } else {
                            return None;
                        }
                    }
                    _ => None,
                };

                let elt_type = Type::common_type(&t1_arr.elt_type, &t2_arr.elt_type)?;
                Some(Arc::new(Type::AnonArray(AnonArrayType { size, elt_type })))
            }

            // An array and a non array. Try to promote the non-array to the array
            (
                other,
                Type::Array(ArrayType {
                    anon_array: arr, ..
                })
                | Type::AnonArray(arr),
            )
            | (
                Type::Array(ArrayType {
                    anon_array: arr, ..
                })
                | Type::AnonArray(arr),
                other,
            ) => {
                if other.is_promotable_to_array() {
                    // Treat the 'other' type as an element of the array
                    let elt_type = Type::common_type(&Arc::new(other.clone()), &arr.elt_type)?;

                    // Promote the single element to an array keeping the same size
                    Some(Arc::new(Type::AnonArray(AnonArrayType {
                        elt_type,
                        size: arr.size,
                    })))
                } else {
                    None
                }
            }

            // Struct -> Struct
            (
                Type::Struct(StructType {
                    anon_struct: t1_struct,
                    ..
                })
                | Type::AnonStruct(t1_struct),
                Type::Struct(StructType {
                    anon_struct: t2_struct,
                    ..
                })
                | Type::AnonStruct(t2_struct),
            ) => {
                // For each member in t1 and t2:
                // - If the member only exists in t1, bring it in unchanged
                // - If the member only exists in t2, bring it in unchanged
                // - If the member exists in _both_, find the common type of the member on both
                //    - If there is no common type, return None
                let mut out_members = HashMap::default();

                for (name, t1_ty) in &t1_struct.members {
                    match t2_struct.members.get(name) {
                        None => {
                            out_members.insert(name.clone(), t1_ty.clone());
                        }
                        Some(t2_ty) => {
                            let member_common = Type::common_type(t1_ty, t2_ty)?;
                            out_members.insert(name.clone(), member_common);
                        }
                    }
                }

                // Add the remaining members left over in t2
                for (name, t2_ty) in &t2_struct.members {
                    if !t1_struct.members.contains_key(name) {
                        out_members.insert(name.clone(), t2_ty.clone());
                    }
                }

                Some(Arc::new(Type::AnonStruct(AnonStructType {
                    members: out_members,
                })))
            }

            // A struct and a non struct. The non struct can fill every member in the struct
            (
                other,
                Type::Struct(StructType {
                    anon_struct: str, ..
                })
                | Type::AnonStruct(str),
            )
            | (
                Type::Struct(StructType {
                    anon_struct: str, ..
                })
                | Type::AnonStruct(str),
                other,
            ) => {
                if other.is_promotable_to_struct() {
                    // Build a new struct with the same members as the old one while trying
                    // to find the common type between the single element and all the members
                    let mut out_members = HashMap::default();
                    let other_rc = Arc::new(other.clone());

                    for (name, in_member_ty) in &str.members {
                        let out_member_ty = Type::common_type(&other_rc, in_member_ty)?;
                        out_members.insert(name.clone(), out_member_ty);
                    }

                    // Create a new struct with similar shape of the old struct
                    Some(Arc::new(Type::AnonStruct(AnonStructType {
                        members: out_members,
                    })))
                } else {
                    None
                }
            }

            _ => None,
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::PrimitiveInt(kind) => Debug::fmt(kind, f),
            Type::Float(kind) => Debug::fmt(kind, f),
            Type::String(_) => f.write_str("string"),
            Type::Boolean => f.write_str("boolean"),
            Type::Integer => f.write_str("Integer"),
            Type::AbsType(ty) => f.write_str(&ty.node.name.data),
            Type::AliasType(ty) => f.write_str(&ty.node.name.data),
            Type::Array(arr) => f.write_str(&arr.node.name.data),
            Type::AnonArray(anon_arr) => {
                match anon_arr.size {
                    None => f.write_str("[] ")?,
                    Some(size) => f.write_fmt(format_args!("[{}] ", size))?,
                }

                Display::fmt(&anon_arr.elt_type, f)
            }
            Type::Enum(ty) => f.write_str(&ty.node.name.data),
            Type::Struct(ty) => f.write_str(&ty.node.name.data),
            Type::AnonStruct(anon_struct) => {
                let mut s = f.debug_struct("anonymous struct");
                let mut members: Vec<(String, Arc<Type>)> =
                    anon_struct.members.clone().into_iter().collect();
                members.sort_by(|a, b| a.0.cmp(&b.0));
                for (name, member_ty) in &members {
                    s.field(name, member_ty.deref());
                }

                s.finish()
            }
        }
    }
}

#[derive(Debug)]
pub enum TypeConversionError {
    ArraySizeMismatch {
        from: usize,
        to: usize,
    },
    ArrayElementDuringPromotion(Box<TypeConversionError>),
    ArrayElement(Box<TypeConversionError>),
    NotPromotableToArray(Box<Type>),
    NotPromotableToStruct(Box<Type>),
    MissingStructMember(String),
    StructMember {
        name: String,
        err: Box<TypeConversionError>,
    },
    Mismatch {
        from: Box<Type>,
        to: Box<Type>,
    },
}

impl TypeConversionError {
    pub fn annotate(&self, diagnostic: Diagnostic) -> Diagnostic {
        match self {
            TypeConversionError::ArraySizeMismatch { from, to } => {
                diagnostic.note(format!("array sizes do not match {} != {}", from, to))
            }
            TypeConversionError::ArrayElement(err) => {
                err.annotate(diagnostic.note("array element type cannot be converted"))
            }
            TypeConversionError::ArrayElementDuringPromotion(err) => {
                err.annotate(diagnostic.note("single element could not be promoted to array"))
            }
            TypeConversionError::NotPromotableToArray(ty) => {
                diagnostic.note(format!("{} cannot be promoted to array", ty))
            }
            TypeConversionError::NotPromotableToStruct(ty) => {
                diagnostic.note(format!("{} cannot be promoted to struct", ty))
            }
            TypeConversionError::MissingStructMember(name) => {
                diagnostic.note(format!("struct missing member `{}`", name))
            }
            TypeConversionError::StructMember { name, err } => err.annotate(
                diagnostic.note(format!("struct member `{}` type cannot be converted", name)),
            ),
            TypeConversionError::Mismatch { from, to } => {
                diagnostic.note(format!("{} cannot be converted to {}", from, to))
            }
        }
    }
}

pub type TypeConversionResult = Result<(), TypeConversionError>;

/// Primitive types
pub trait PrimitiveType {
    fn bit_width(&self) -> u32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveIntSignedness {
    Signed,
    Unsigned,
}

pub fn int_kind_signedness(kind: IntegerKind) -> PrimitiveIntSignedness {
    match kind {
        IntegerKind::I8 | IntegerKind::I16 | IntegerKind::I32 | IntegerKind::I64 => {
            PrimitiveIntSignedness::Signed
        }
        IntegerKind::U8 | IntegerKind::U16 | IntegerKind::U32 | IntegerKind::U64 => {
            PrimitiveIntSignedness::Unsigned
        }
    }
}

impl PrimitiveType for IntegerKind {
    fn bit_width(&self) -> u32 {
        match self {
            IntegerKind::I8 => 8,
            IntegerKind::U8 => 8,
            IntegerKind::I16 => 16,
            IntegerKind::U16 => 16,
            IntegerKind::I32 => 32,
            IntegerKind::U32 => 32,
            IntegerKind::I64 => 64,
            IntegerKind::U64 => 64,
        }
    }
}

impl PrimitiveType for FloatKind {
    fn bit_width(&self) -> u32 {
        match self {
            FloatKind::F32 => 32,
            FloatKind::F64 => 64,
        }
    }
}

/// An abstract type
#[derive(Debug, Clone)]
pub struct AbsType {
    /// The AST node giving the definition
    pub node: fpp_ast::DefAbsType,
    pub default_value: Option<AbsTypeValue>,
}

/// An alias type
#[derive(Debug, Clone)]
pub struct AliasType {
    /// The AST node giving the definition
    pub node: fpp_ast::DefAliasType,
    /// Type that this typedef points to
    pub alias_type: Arc<Type>,
}

/// A named array type
#[derive(Debug, Clone)]
pub struct ArrayType {
    /// The AST node giving the definition
    pub node: fpp_ast::DefArray,
    /// The structurally equivalent anonymous array
    pub anon_array: AnonArrayType,
    /// The specified default value, if any
    pub default: Option<Value>,
    /// The specified format, if any
    pub format: Option<Format>,
}

/// An anonymous array type
#[derive(Debug, Clone)]
pub struct AnonArrayType {
    /// The array size
    pub size: Option<usize>,
    /// The element type
    pub elt_type: Arc<Type>,
}

/// An enum type
#[derive(Debug, Clone)]
pub struct EnumType {
    /// The AST node giving the definition
    pub node: fpp_ast::DefEnum,
    /// The representation type
    pub rep_type: IntegerKind,
    /// The default value
    pub default: Option<Value>,
}

/// A named struct type
#[derive(Debug, Clone)]
pub struct StructType {
    /// The AST node giving the definition
    pub node: fpp_ast::DefStruct,
    /// The structurally equivalent anonymous struct type
    pub anon_struct: AnonStructType,
    /// The default value
    pub default: Option<StructValue>,
    /// The member sizes
    pub sizes: HashMap<String, u32>,
    /// The member formats
    pub formats: HashMap<String, Format>,
}

/// An anonymous struct type
#[derive(Debug, Clone)]
pub struct AnonStructType {
    /// The members
    pub members: HashMap<String, Arc<Type>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string(size: Option<i128>) -> Arc<Type> {
        Arc::new(Type::String(size))
    }

    fn anon_array(elt: Arc<Type>, size: Option<usize>) -> Arc<Type> {
        Arc::new(Type::AnonArray(AnonArrayType {
            elt_type: elt,
            size,
        }))
    }

    /// Regression for the `common_type` enum arm (fidelity bug #1): the enum
    /// case must recurse on `(rep_type, t2)`, not `(rep_type, t1)`, so the
    /// *other* operand is not dropped. `enum + F64` must widen to `F64`.
    #[test]
    fn common_type_enum_and_float() {
        // Build a minimal enum type (needs a node span, hence a context).
        let mut buf = vec![];
        let mut ctx = fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut buf));
        fpp_core::run(&mut ctx, || {
            let src = fpp_core::SourceFile::new("test", "enum E { X }".to_string());
            let span = fpp_core::Span::new(src, 0, 1, None);
            let def_enum = fpp_ast::DefEnum {
                name: fpp_ast::Name {
                    data: "E".to_string(),
                    node_id: fpp_core::Node::new(span),
                },
                type_name: None,
                constants: vec![],
                default: None,
                is_dictionary_def: false,
                node_id: fpp_core::Node::new(span),
            };
            let enum_ty = Arc::new(Type::Enum(EnumType {
                node: def_enum,
                rep_type: IntegerKind::I32,
                default: None,
            }));
            let f64_ty = Arc::new(Type::Float(FloatKind::F64));

            // enum + F64 -> F64 (widens through the rep type to the float)
            let ct = Type::common_type(&enum_ty, &f64_ty).expect("common type");
            assert!(ct.is_float(), "expected F64, got {ct}");
            // symmetric
            let ct = Type::common_type(&f64_ty, &enum_ty).expect("common type");
            assert!(ct.is_float(), "expected F64, got {ct}");
        });
    }

    /// Regression for `identical` (fidelity bug #6): two equal fixed-size strings
    /// are NOT identical (there is no `String` case in the identity check), so
    /// their common type is the unsized `String(None)`.
    #[test]
    fn identical_strings_are_not_identical() {
        assert!(!Type::identical(
            &Type::String(Some(8)),
            &Type::String(Some(8))
        ));
        let ct = Type::common_type(&string(Some(8)), &string(Some(8))).expect("common type");
        assert!(
            matches!(ct.deref(), Type::String(None)),
            "expected String(None), got {ct}"
        );
    }

    /// Regression for `is_displayable` (fidelity bug #7): anonymous aggregates
    /// (`AnonArray`/`AnonStruct`) inherit the base `false` even when their
    /// elements are displayable.
    #[test]
    fn anon_aggregates_are_not_displayable() {
        assert!(
            !anon_array(Arc::new(Type::Boolean), Some(3)).is_displayable(),
            "anonymous array must not be displayable"
        );
        let mut members = HashMap::default();
        members.insert("x".to_string(), Arc::new(Type::Boolean));
        assert!(
            !Type::AnonStruct(AnonStructType { members }).is_displayable(),
            "anonymous struct must not be displayable"
        );
    }
}
