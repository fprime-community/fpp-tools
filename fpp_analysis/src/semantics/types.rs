use crate::semantics::{
    AbsTypeValue, AnonArrayValue, AnonStructValue, ArrayValue, BooleanValue, EnumConstantValue,
    FloatValue, Format, IntegerValue, PrimitiveIntegerValue, StringValue, StructValue, Symbol,
    Value,
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
    Abs(Arc<AbsType>),
    Alias(AliasType),
    Array(Arc<ArrayType>),
    AnonArray(AnonArrayType),
    Enum(Arc<EnumType>),
    Struct(Arc<StructType>),
    AnonStruct(AnonStructType),
}

/// Why [`Type::serialized_size`] could not produce a size
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SerializedSizeError {
    /// The size depends on information that is not there: a framework definition
    /// that is not defined, a type that is not finalized, or a type with no
    /// serialized representation at all (`Integer`, an abstract type).
    Unavailable,
    /// The size does not fit in an `i128`. Sizes are represented exactly here, so
    /// an inexpressible size is reported rather than silently wrapped.
    TooLarge,
}

impl Type {
    /// Get the underlying type
    pub fn underlying_type(ty: &Arc<Type>) -> Arc<Type> {
        match ty.deref() {
            Type::Alias(alias) => Type::underlying_type(&alias.alias_type),
            _ => ty.clone(),
        }
    }

    /// Get the default value
    pub fn default_value(self: &Arc<Type>) -> Option<Value> {
        match self.deref() {
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
            Type::Alias(ty) => ty.alias_type.default_value(),
            // An abstract type always has a default value: the opaque value of
            // the type itself.
            Type::Abs(ty) => Some(Value::AbsType(AbsTypeValue { ty: ty.clone() })),
            Type::Array(array) => array.default.clone().map(Value::Array),
            // `size` copies of the element default, sharing one element: the
            // element default is built once and the array holds `size`
            // references to it, so a nested array default costs the SUM of the
            // nested sizes rather than their PRODUCT. That matters because a
            // legal array size is `1..=i32::MAX`
            // (`FinalizeTypeDefs::visit_def_array` rejects anything outside the
            // `i32` range with "value out of range" and anything `<= 0`) and
            // array element types nest, so
            //
            //     array Inner = [n] U8
            //     array Outer = [n] Inner
            //
            // holds 2n references rather than n^2 values.
            Type::AnonArray(arr) => Some(Value::AnonArray(AnonArrayValue::repeated(
                arr.elt_type.default_value()?,
                arr.size?,
            ))),
            Type::Enum(ty) => ty.default.clone().map(Value::EnumConstant),
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

    /// The anonymous array structure of an array type, named or not
    pub fn as_anon_array(&self) -> Option<&AnonArrayType> {
        match self {
            Type::Array(ty) => Some(&ty.anon_array),
            Type::AnonArray(ty) => Some(ty),
            _ => None,
        }
    }

    /// The anonymous struct structure of a struct type, named or not
    pub fn as_anon_struct(&self) -> Option<&AnonStructType> {
        match self {
            Type::Struct(ty) => Some(&ty.anon_struct),
            Type::AnonStruct(ty) => Some(ty),
            _ => None,
        }
    }

    /// Get the array size
    pub fn array_size(&self) -> Option<usize> {
        match self {
            Type::Alias(ty) => ty.alias_type.array_size(),
            Type::AnonArray(arr) => arr.size,
            Type::Array(arr) => arr.anon_array.size,
            _ => None,
        }
    }

    /// Get the definition symbol, if any
    ///
    /// The definition is held behind an `Arc`, so this is a reference-count bump
    /// and no AST is copied; callers may use the result as a map key in a loop.
    pub fn def_symbol(&self) -> Option<Symbol> {
        match self {
            Type::Abs(ty) => Some(Symbol::AbsType(ty.node.clone())),
            Type::Alias(ty) => Some(Symbol::AliasType(ty.node.clone())),
            Type::Array(ty) => Some(Symbol::ArrayType(ty.node.clone())),
            Type::Enum(ty) => Some(Symbol::EnumType(ty.node.clone())),
            Type::Struct(ty) => Some(Symbol::StructType(ty.node.clone())),
            _ => None,
        }
    }

    /// Get the definition node identifier, if any
    pub fn def_node_id(&self) -> Option<fpp_core::Node> {
        match self {
            Type::Abs(ty) => Some(ty.node.node_id),
            Type::Alias(ty) => Some(ty.node.node_id),
            Type::Array(ty) => Some(ty.node.node_id),
            Type::Enum(ty) => Some(ty.node.node_id),
            Type::Struct(ty) => Some(ty.node.node_id),
            _ => None,
        }
    }

    /// Does this type have numeric members?
    pub fn has_numeric_members(&self) -> bool {
        match self {
            Type::Alias(ty) => ty.alias_type.has_numeric_members(),
            Type::Array(ty) => ty.anon_array.elt_type.has_numeric_members(),
            Type::AnonArray(ty) => ty.elt_type.has_numeric_members(),
            Type::Struct(ty) => ty
                .anon_struct
                .members
                .iter()
                .all(|(_, member)| member.has_numeric_members()),
            Type::AnonStruct(ty) => ty
                .members
                .iter()
                .all(|(_, member)| member.has_numeric_members()),
            _ => self.is_numeric(),
        }
    }

    /// Is this type convertible to a numeric type?
    pub fn is_convertible_to_numeric(&self) -> bool {
        match self {
            Type::Alias(ty) => ty.alias_type.is_convertible_to_numeric(),
            Type::Enum(_) => true,
            _ => self.is_numeric(),
        }
    }

    /// Is this type promotable to an array type?
    pub fn is_promotable_to_array(&self) -> bool {
        match self {
            Type::Alias(ty) => ty.alias_type.is_promotable_to_array(),
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
            Type::Abs(_) => false,
            Type::Alias(alias) => alias.alias_type.is_displayable(),
            // A named array is displayable iff its element type is; anonymous
            // aggregates (AnonArray/AnonStruct) inherit the base `false`.
            Type::Array(arr) => arr.anon_array.elt_type.is_displayable(),
            Type::AnonArray(_) => false,
            Type::Enum(_) => true,
            Type::Struct(ty) => ty
                .anon_struct
                .members
                .iter()
                .all(|(_, member)| member.is_displayable()),
            Type::AnonStruct(_) => false,
        }
    }

    /// Is this type a float type?
    pub fn is_float(&self) -> bool {
        match self {
            Type::Alias(ty) => ty.alias_type.is_float(),
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
            Type::Alias(alias) => alias.alias_type.primitive_serialized_size(),
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
    /// sizes known).
    ///
    /// The size is a product over the nesting depth of the type, so it grows
    /// faster than any single array size: 128 levels of `array A = [2] B` reach
    /// 2^128 bytes. An `i128` cannot hold that, so a size that does not fit is
    /// reported as [`SerializedSizeError::TooLarge`] rather than wrapped.
    pub fn serialized_size(&self, a: &crate::Analysis) -> Result<i128, SerializedSizeError> {
        use crate::semantics::SymbolInterface;
        use SerializedSizeError::Unavailable;

        /// `n * size`, or [`SerializedSizeError::TooLarge`]
        fn scale(n: usize, size: i128) -> Result<i128, SerializedSizeError> {
            (n as i128)
                .checked_mul(size)
                .ok_or(SerializedSizeError::TooLarge)
        }

        match self {
            Type::Alias(alias) => alias.alias_type.serialized_size(a),
            Type::Boolean => Ok(1),
            Type::Float(FloatKind::F32) => Ok(4),
            Type::Float(FloatKind::F64) => Ok(8),
            Type::PrimitiveInt(_) => self.primitive_serialized_size().ok_or(Unavailable),
            Type::Enum(ty) => Type::PrimitiveInt(ty.rep_type).serialized_size(a),
            Type::Array(arr) => {
                let n = arr.anon_array.size.ok_or(Unavailable)?;
                scale(n, arr.anon_array.elt_type.serialized_size(a)?)
            }
            Type::AnonArray(arr) => {
                let n = arr.size.ok_or(Unavailable)?;
                scale(n, arr.elt_type.serialized_size(a)?)
            }
            Type::String(size) => {
                let store_symbol = a
                    .framework_definitions
                    .type_map
                    .get("FwSizeStoreType")
                    .ok_or(Unavailable)?;
                let store_size = a
                    .type_map
                    .get(&store_symbol.node())
                    .ok_or(Unavailable)?
                    .serialized_size(a)?;
                let data_size = match size {
                    Some(n) => *n,
                    None => {
                        let c = a
                            .framework_definitions
                            .constant_map
                            .get("FW_FIXED_LENGTH_STRING_SIZE")
                            .ok_or(Unavailable)?;
                        a.get_int_value(c.node()).ok_or(Unavailable)?
                    }
                };
                store_size
                    .checked_add(data_size)
                    .ok_or(SerializedSizeError::TooLarge)
            }
            Type::Struct(ty) => {
                let mut total = 0i128;
                for (name, member_ty) in &ty.anon_struct.members {
                    let member_size = member_ty.serialized_size(a)?;
                    let mult = ty.sizes.get(name).copied().unwrap_or(1);
                    total = total
                        .checked_add(scale(mult as usize, member_size)?)
                        .ok_or(SerializedSizeError::TooLarge)?;
                }
                Ok(total)
            }
            Type::AnonStruct(ty) => {
                let mut total = 0i128;
                for (_, member_ty) in &ty.members {
                    total = total
                        .checked_add(member_ty.serialized_size(a)?)
                        .ok_or(SerializedSizeError::TooLarge)?;
                }
                Ok(total)
            }
            Type::Integer | Type::Abs(_) => Err(Unavailable),
        }
    }

    /// Is this type an int type?
    pub fn is_int(&self) -> bool {
        match self {
            Type::Alias(ty) => ty.alias_type.is_int(),
            Type::PrimitiveInt(_) => true,
            Type::Integer => true,
            _ => false,
        }
    }

    /// Is this type a primitive type?
    pub fn is_primitive(&self) -> bool {
        match self {
            Type::Alias(ty) => ty.alias_type.is_primitive(),
            Type::PrimitiveInt(_) => true,
            Type::Float(_) => true,
            Type::Boolean => true,
            _ => false,
        }
    }

    /// Is this type a canonical (non-aliased) type?
    pub fn is_canonical(&self) -> bool {
        !matches!(self, Type::Alias(_))
    }

    /// Is this type promotable to a struct type?
    pub fn is_promotable_to_struct(&self) -> bool {
        self.is_promotable_to_array()
    }

    /// Is this type numeric?
    pub fn is_numeric(&self) -> bool {
        match self {
            Type::Alias(ty) => ty.alias_type.is_numeric(),
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

        // String -> String
        if let (Type::String(_), Type::String(_)) = (from, to) {
            return Ok(());
        }

        // Array -> Array
        if let (Some(from_arr), Some(to_arr)) = (from.as_anon_array(), to.as_anon_array()) {
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

            return match Type::convert(&from_arr.elt_type, &to_arr.elt_type) {
                Ok(()) => Ok(()),
                Err(err) => Err(TypeConversionError::ArrayElement(Box::new(err))),
            };
        }

        // Convert a single element to an array
        if let Some(to_arr) = to.as_anon_array() {
            if !from.is_promotable_to_array() {
                return Err(TypeConversionError::NotPromotableToArray(Box::new(
                    from.clone(),
                )));
            }

            return match Type::convert_impl(from, Type::underlying_type(&to_arr.elt_type).deref()) {
                Ok(_) => Ok(()),
                Err(err) => Err(TypeConversionError::ArrayElementDuringPromotion(Box::new(
                    err,
                ))),
            };
        }

        // Struct -> Struct
        if let (Some(from_struct), Some(to_struct)) = (from.as_anon_struct(), to.as_anon_struct()) {
            // May sure all the members in 'from' can fit into 'to'
            for (name, from_member_ty) in &from_struct.members {
                match to_struct.get_member(name) {
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

            return Ok(());
        }

        // Convert a single element to a struct
        if let Some(to_struct) = to.as_anon_struct() {
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

            return Ok(());
        }

        Err(TypeConversionError::Mismatch {
            from: Box::new(from.clone()),
            to: Box::new(to.clone()),
        })
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
                    if let Type::Alias(AliasType { alias_type, .. }) = t.deref() {
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
            (Type::String(_), Type::String(_)) => return Some(Arc::new(Type::String(None))),

            // Strip off any enum wrappers over the representable type
            (Type::Enum(ty), _) => {
                return Self::common_type(&Arc::new(Type::PrimitiveInt(ty.rep_type)), &t2);
            }
            (_, Type::Enum(ty)) => {
                return Self::common_type(&t1, &Arc::new(Type::PrimitiveInt(ty.rep_type)));
            }

            _ => {}
        }

        // t1 + t2 are both array/anon array
        if let (Some(t1_arr), Some(t2_arr)) = (t1.as_anon_array(), t2.as_anon_array()) {
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
            return Some(Arc::new(Type::AnonArray(AnonArrayType { size, elt_type })));
        }

        // An array and a non array. Try to promote the non-array to the array
        if let Some((arr, other)) = match (t1.as_anon_array(), t2.as_anon_array()) {
            (Some(arr), None) => Some((arr, t2.deref())),
            (None, Some(arr)) => Some((arr, t1.deref())),
            _ => None,
        } {
            if !other.is_promotable_to_array() {
                return None;
            }
            // Treat the 'other' type as an element of the array
            let elt_type = Type::common_type(&Arc::new(other.clone()), &arr.elt_type)?;

            // Promote the single element to an array keeping the same size
            return Some(Arc::new(Type::AnonArray(AnonArrayType {
                elt_type,
                size: arr.size,
            })));
        }

        // Struct -> Struct
        if let (Some(t1_struct), Some(t2_struct)) = (t1.as_anon_struct(), t2.as_anon_struct()) {
            // For each member in t1 and t2:
            // - If the member only exists in t1, bring it in unchanged
            // - If the member only exists in t2, bring it in unchanged
            // - If the member exists in _both_, find the common type of the member on both
            //    - If there is no common type, return None
            let mut out_members = Vec::default();

            for (name, t1_ty) in &t1_struct.members {
                match t2_struct.get_member(name) {
                    None => {
                        out_members.push((name.clone(), t1_ty.clone()));
                    }
                    Some(t2_ty) => {
                        let member_common = Type::common_type(t1_ty, t2_ty)?;
                        out_members.push((name.clone(), member_common));
                    }
                }
            }

            // Add the remaining members left over in t2
            for (name, t2_ty) in &t2_struct.members {
                if !t1_struct.has_member(name) {
                    out_members.push((name.clone(), t2_ty.clone()));
                }
            }

            return Some(Arc::new(Type::AnonStruct(AnonStructType {
                members: out_members,
            })));
        }

        // A struct and a non struct. The non struct can fill every member in the struct
        if let Some((str, other)) = match (t1.as_anon_struct(), t2.as_anon_struct()) {
            (Some(str), None) => Some((str, t2.deref())),
            (None, Some(str)) => Some((str, t1.deref())),
            _ => None,
        } {
            if !other.is_promotable_to_struct() {
                return None;
            }
            // Build a new struct with the same members as the old one while trying
            // to find the common type between the single element and all the members
            let mut out_members = Vec::default();
            let other_rc = Arc::new(other.clone());

            for (name, in_member_ty) in &str.members {
                let out_member_ty = Type::common_type(&other_rc, in_member_ty)?;
                out_members.push((name.clone(), out_member_ty));
            }

            // Create a new struct with similar shape of the old struct
            return Some(Arc::new(Type::AnonStruct(AnonStructType {
                members: out_members,
            })));
        }

        None
    }
}

/// A type rendered by its [`Display`] inside a `Debug` builder, which is what
/// [`Formatter::debug_struct`]'s `field` takes.
struct Displayed<'a>(&'a Type);

impl Debug for Displayed<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.0, f)
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
            Type::Abs(ty) => f.write_str(&ty.node.name.data),
            Type::Alias(ty) => f.write_str(&ty.node.name.data),
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
                    s.field(name, &Displayed(member_ty.deref()));
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
    pub node: Arc<fpp_ast::DefAbsType>,
}

/// An alias type
#[derive(Debug, Clone)]
pub struct AliasType {
    /// The AST node giving the definition
    pub node: Arc<fpp_ast::DefAliasType>,
    /// Type that this typedef points to
    pub alias_type: Arc<Type>,
}

/// A named array type
#[derive(Debug, Clone)]
pub struct ArrayType {
    /// The AST node giving the definition
    pub node: Arc<fpp_ast::DefArray>,
    /// The structurally equivalent anonymous array
    pub anon_array: AnonArrayType,
    /// The specified default value, if any
    pub default: Option<ArrayValue>,
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
    pub node: Arc<fpp_ast::DefEnum>,
    /// The representation type
    pub rep_type: IntegerKind,
    /// The default value
    pub default: Option<EnumConstantValue>,
}

/// A named struct type
#[derive(Debug, Clone)]
pub struct StructType {
    /// The AST node giving the definition
    pub node: Arc<fpp_ast::DefStruct>,
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
    pub members: Vec<(String, Arc<Type>)>,
}

impl AnonStructType {
    pub fn get_member(&self, name: &str) -> Option<&Arc<Type>> {
        self.members
            .iter()
            .find_map(|(k, v)| if k == name { Some(v) } else { None })
    }

    pub fn has_member(&self, name: &str) -> bool {
        self.get_member(name).is_some()
    }
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

    fn prim_int(kind: IntegerKind) -> Arc<Type> {
        Arc::new(Type::PrimitiveInt(kind))
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
            let def_enum = Arc::new(fpp_ast::DefEnum {
                name: fpp_ast::Name {
                    data: "E".to_string(),
                    node_id: fpp_core::Node::new(span),
                },
                type_name: None,
                constants: vec![],
                default: None,
                is_dictionary_def: false,
                node_id: fpp_core::Node::new(span),
            });
            let enum_ty = Arc::new(Type::Enum(Arc::new(EnumType {
                node: def_enum,
                rep_type: IntegerKind::I32,
                default: None,
            })));
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

    /// An array default holds `size` references to one element value, so a
    /// nested array default costs the SUM of the nested sizes and not their
    /// PRODUCT.
    ///
    /// The pointer-identity assertions are what bound the memory: with an
    /// independent copy per element, `array Inner = [n] U8` +
    /// `array Outer = [n] Inner` builds n^2 values of 64 bytes each (measured
    /// peak RSS 1.1 GB at n=4000 and 4.2 GB at n=8000), and at the size bound
    /// (n = 2^31-1) it cannot complete.
    #[test]
    fn array_default_shares_its_repeated_element() {
        const N: usize = 2000;

        let inner = anon_array(prim_int(IntegerKind::U8), Some(N));
        let outer = anon_array(inner, Some(N));

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
        let Value::AnonArray(inner_default) = first.deref() else {
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
        assert!(matches!(
            inner_first.deref(),
            Value::PrimitiveInteger(PrimitiveIntegerValue {
                value: 0,
                kind: IntegerKind::U8
            })
        ));

        // Reading the value is unaffected by the sharing
        assert_eq!(inner_default.iter().count(), N);
        assert!(inner_default.iter().all(|e| e.to_string() == "0"));
        assert!(inner_default.get(N - 1).is_some());
        assert!(inner_default.get(N).is_none());
    }

    /// A scalar promoted to an array of known size shares the promoted element
    /// too, and an array of unknown size records it as the array's scalar.
    #[test]
    fn promoted_scalar_shares_its_element() {
        const N: usize = 1000;
        let scalar = Value::PrimitiveInteger(PrimitiveIntegerValue {
            value: 7,
            kind: IntegerKind::U32,
        });

        let u32_array = |size| anon_array(prim_int(IntegerKind::U32), size);
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
        let u8_array = anon_array(prim_int(IntegerKind::U8), Some(N));
        let u32_array = anon_array(prim_int(IntegerKind::U32), Some(N));
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

    /// Regression for `is_displayable` (fidelity bug #7): anonymous aggregates
    /// (`AnonArray`/`AnonStruct`) inherit the base `false` even when their
    /// elements are displayable.
    #[test]
    fn anon_aggregates_are_not_displayable() {
        assert!(
            !anon_array(Arc::new(Type::Boolean), Some(3)).is_displayable(),
            "anonymous array must not be displayable"
        );
        let mut members = Vec::default();
        members.push(("x".to_string(), Arc::new(Type::Boolean)));
        assert!(
            !Type::AnonStruct(AnonStructType { members }).is_displayable(),
            "anonymous struct must not be displayable"
        );
    }
}
