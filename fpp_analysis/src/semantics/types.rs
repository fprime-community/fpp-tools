use crate::semantics::{
    AbsTypeValue, AnonArrayValue, AnonStructValue, ArrayValue, BooleanValue, EnumConstantValue,
    FloatValue, Format, IntegerValue, PrimitiveIntegerValue, StringValue, StructValue, Symbol,
    SymbolInterface, TypeVisitor, Value,
};
use fpp_ast::{FloatKind, IntegerKind};
use fpp_core::Diagnostic;
use indexmap::IndexMap;
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
    /// The type of a string, with its size. The size is the value of the size
    /// expression of a `string size e` type name, which `FinalizeTypeDefs`
    /// evaluates into the type; before that, and for a string with no size, it
    /// is `None`.
    String(Option<i128>),
    /// The Boolean type
    Boolean,
    /// The type of arbitrary-width integers
    Integer,
    /// An abstract type
    Abs(Arc<AbsType>),
    /// An alias type
    Alias(AliasType),
    /// A named array type
    Array(Arc<ArrayType>),
    /// An anonymous array type
    AnonArray(AnonArrayType),
    /// An enum type
    Enum(Arc<EnumType>),
    /// A named struct type
    Struct(Arc<StructType>),
    /// An anonymous struct type
    AnonStruct(AnonStructType),
}

impl Type {
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
                    members: IndexMap::from_iter(members),
                }))
            }
        }
    }

    /// Get the array size
    pub fn array_size(&self) -> Option<usize> {
        match self {
            Type::AnonArray(arr) => arr.size,
            Type::Array(arr) => arr.anon_array.size,
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

    /// Get the underlying type
    pub fn underlying_type(ty: &Arc<Type>) -> Arc<Type> {
        match ty.deref() {
            Type::Alias(alias) => Type::underlying_type(&alias.alias_type),
            _ => ty.clone(),
        }
    }

    /// Does this type have numeric members?
    ///
    /// A type has numeric members if its underlying type does (spec, Types
    /// with Numeric Members), so this predicate looks through an alias type.
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
            Type::Enum(_) => true,
            _ => self.is_numeric(),
        }
    }

    /// Is this type promotable to an array type?
    pub fn is_promotable_to_array(&self) -> bool {
        match self {
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
        matches!(self, Type::Float(_))
    }

    /// Is this type an int type?
    pub fn is_int(&self) -> bool {
        matches!(self, Type::PrimitiveInt(_) | Type::Integer)
    }

    /// Is this type a primitive type?
    pub fn is_primitive(&self) -> bool {
        matches!(self, Type::PrimitiveInt(_) | Type::Float(_) | Type::Boolean)
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
        self.is_int() || self.is_float()
    }

    /// Is this type convertible to type t?
    pub fn is_convertible_to(self: &Arc<Type>, t: &Arc<Type>) -> bool {
        Type::convert(self, t).is_ok()
    }

    // Rust helpers

    /// Whether a size is valid for a string type: a nonnegative value that fits
    /// in an `i32`
    pub fn is_valid_string_size(size: i128) -> bool {
        size >= 0 && size <= i32::MAX as i128
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

    /// The serialized size, in bytes, of a type whose size does not depend on
    /// framework definitions or on finalization: a primitive type, or an alias
    /// or enum of one. `None` for any other type; [`Type::serialized_size`]
    /// computes the size of every type that has one.
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

    /// Computes the serialized size of a type, as the `SerializedSize` type
    /// visitor does
    pub fn serialized_size(&self, a: &crate::Analysis) -> Result<i128, SerializedSizeError> {
        SerializedSize.ty(a, self)
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::PrimitiveInt(kind) => Debug::fmt(kind, f),
            Type::Float(kind) => Debug::fmt(kind, f),
            Type::String(_) => f.write_str("string"),
            Type::Boolean => f.write_str("bool"),
            Type::Integer => f.write_str("Integer"),
            Type::Abs(ty) => f.write_str(&ty.node.name.data),
            Type::Alias(ty) => f.write_str(&ty.node.name.data),
            Type::Array(arr) => write!(f, "array {}", arr.node.name.data),
            Type::AnonArray(anon_arr) => match anon_arr.size {
                Some(n) => write!(f, "[{}] {}", n, anon_arr.elt_type),
                None => write!(f, "array of {}", anon_arr.elt_type),
            },
            Type::Enum(ty) => write!(f, "enum {}", ty.node.name.data),
            Type::Struct(ty) => write!(f, "struct {}", ty.node.name.data),
            Type::AnonStruct(anon_struct) => match anon_struct.members.len() {
                0 => f.write_str("{ }"),
                _ => {
                    f.write_str("{ ")?;
                    for (i, (name, member_ty)) in anon_struct.members.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        write!(f, "{}: {}", name, member_ty)?;
                    }
                    f.write_str(" }")
                }
            },
        }
    }
}

/// Primitive types
pub trait Primitive {
    /// The width of the type, in bits
    fn bit_width(&self) -> u32;
}

/// Primitive integer types, given by their kinds
pub trait PrimitiveInt: Primitive {
    /// The signedness of the type
    fn signedness(&self) -> Signedness;
}

/// The signedness of a primitive integer type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signedness {
    Signed,
    Unsigned,
}

impl PrimitiveInt for IntegerKind {
    fn signedness(&self) -> Signedness {
        match self {
            IntegerKind::I8 | IntegerKind::I16 | IntegerKind::I32 | IntegerKind::I64 => {
                Signedness::Signed
            }
            IntegerKind::U8 | IntegerKind::U16 | IntegerKind::U32 | IntegerKind::U64 => {
                Signedness::Unsigned
            }
        }
    }
}

impl Primitive for IntegerKind {
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

impl Primitive for FloatKind {
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

impl ArrayType {
    /// Check whether two array sizes match
    pub fn sizes_match(size1: Option<usize>, size2: Option<usize>) -> bool {
        match (size1, size2) {
            (Some(n1), Some(n2)) => n1 == n2,
            _ => true,
        }
    }

    /// Compute a common array size
    pub fn common_size(size1: Option<usize>, size2: Option<usize>) -> Option<usize> {
        match (size1, size2) {
            (Some(n1), Some(n2)) if n1 == n2 => Some(n1),
            _ => None,
        }
    }
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

/// A member of a struct type: its name and its type
pub(crate) type StructMember = (String, Arc<Type>);

/// The members of a struct type, in order
pub(crate) type StructMembers = Vec<StructMember>;

impl StructType {
    /// Resolve a member map, generating a new member map
    pub(crate) fn resolve_members(
        resolver: impl Fn(&StructMember) -> Option<StructMember>,
        members: &[StructMember],
    ) -> Option<StructMembers> {
        members.iter().map(resolver).collect()
    }
}

/// An anonymous array type
#[derive(Debug, Clone)]
pub struct AnonArrayType {
    /// The array size
    pub size: Option<usize>,
    /// The element type
    pub elt_type: Arc<Type>,
}

/// An anonymous struct type
#[derive(Debug, Clone)]
pub struct AnonStructType {
    /// The members
    pub members: Vec<(String, Arc<Type>)>,
}

impl AnonStructType {
    /// The type of the member named `name`, if there is one
    pub fn get_member(&self, name: &str) -> Option<&Arc<Type>> {
        self.members
            .iter()
            .find_map(|(k, v)| if k == name { Some(v) } else { None })
    }

    /// Whether there is a member named `name`
    pub fn has_member(&self, name: &str) -> bool {
        self.get_member(name).is_some()
    }
}

impl Type {
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

    /// Check for type convertibility: whether `from` is convertible to `to`,
    /// and if not, why not
    pub fn convert(from: &Arc<Type>, to: &Arc<Type>) -> TypeConversionResult {
        Type::may_be_converted(
            Type::underlying_type(from).deref(),
            Type::underlying_type(to).deref(),
        )
    }

    /// Check for type convertibility on a pair of canonical types
    ///
    /// The types are convertible if they are identical, or if one of the
    /// numeric, string, array, and struct rules applies to them and accepts
    /// them. A rule that applies but rejects the pair gives the reason.
    fn may_be_converted(t1: &Type, t2: &Type) -> TypeConversionResult {
        assert!(t1.is_canonical());
        assert!(t2.is_canonical());

        let numeric = || (t1.is_convertible_to_numeric() && t2.is_numeric()).then_some(Ok(()));
        let string = || matches!((t1, t2), (Type::String(_), Type::String(_))).then_some(Ok(()));
        let array = || match (t1.as_anon_array(), t2.as_anon_array()) {
            (Some(anon_array1), Some(anon_array2)) => Some(
                if ArrayType::sizes_match(anon_array1.size, anon_array2.size) {
                    Type::convert(&anon_array1.elt_type, &anon_array2.elt_type)
                        .map_err(|err| TypeConversionError::ArrayElement(Box::new(err)))
                } else {
                    Err(TypeConversionError::ArraySizeMismatch {
                        from: anon_array1.size.expect("an unequal size"),
                        to: anon_array2.size.expect("an unequal size"),
                    })
                },
            ),
            (_, Some(anon_array2)) => Some(if t1.is_promotable_to_array() {
                Type::may_be_converted(t1, Type::underlying_type(&anon_array2.elt_type).deref())
                    .map_err(|err| TypeConversionError::ArrayElementDuringPromotion(Box::new(err)))
            } else {
                Err(TypeConversionError::NotPromotableToArray(Box::new(
                    t1.clone(),
                )))
            }),
            _ => None,
        };
        let struct_ = || {
            // A member of t1 exists in the members of t2, with a type that it
            // converts to
            let member_exists_in =
                |members: &AnonStructType, (name, member_type1): &StructMember| {
                    let Some(member_type2) = members.get_member(name) else {
                        return Err(TypeConversionError::MissingStructMember(name.clone()));
                    };
                    Type::convert(member_type1, member_type2).map_err(|err| {
                        TypeConversionError::StructMember {
                            name: name.clone(),
                            err: Box::new(err),
                        }
                    })
                };
            match (t1.as_anon_struct(), t2.as_anon_struct()) {
                (Some(anon_struct1), Some(anon_struct2)) => Some(
                    anon_struct1
                        .members
                        .iter()
                        .try_for_each(|member| member_exists_in(anon_struct2, member)),
                ),
                (_, Some(anon_struct2)) => Some(if t1.is_promotable_to_struct() {
                    // t1 converts to the type of every member of t2
                    anon_struct2
                        .members
                        .iter()
                        .try_for_each(|(name, member_type2)| {
                            Type::may_be_converted(t1, Type::underlying_type(member_type2).deref())
                                .map_err(|err| TypeConversionError::StructMember {
                                    name: name.clone(),
                                    err: Box::new(err),
                                })
                        })
                } else {
                    Err(TypeConversionError::NotPromotableToStruct(Box::new(
                        t1.clone(),
                    )))
                }),
                _ => None,
            }
        };

        if Type::identical(t1, t2) {
            return Ok(());
        }
        numeric()
            .or_else(string)
            .or_else(array)
            .or_else(struct_)
            .unwrap_or_else(|| {
                Err(TypeConversionError::Mismatch {
                    from: Box::new(t1.clone()),
                    to: Box::new(t2.clone()),
                })
            })
    }

    /// Compute the common type for a pair of types
    ///
    /// The rules are tried in order, and the first one that gives a type wins.
    pub fn common_type(t1_a: &Arc<Type>, t2_a: &Arc<Type>) -> Option<Arc<Type>> {
        let (t1, t2) = (t1_a, t2_a);
        let rules: [CommonTypeRule; 7] = [
            Type::common_type_identical,
            Type::common_type_alias,
            Type::common_type_numeric,
            Type::common_type_string,
            Type::common_type_enumeration,
            Type::common_type_array,
            Type::common_type_struct,
        ];
        rules.iter().find_map(|rule| rule(t1, t2))
    }

    fn common_type_identical(t1: &Arc<Type>, t2: &Arc<Type>) -> Option<Arc<Type>> {
        Type::identical(t1, t2).then(|| t1.clone())
    }

    fn common_type_alias(t1: &Arc<Type>, t2: &Arc<Type>) -> Option<Arc<Type>> {
        fn lca(a: &Arc<Type>, b: &Arc<Type>) -> Option<Arc<Type>> {
            /// The ancestors of a type in the alias hierarchy, youngest first
            fn get_ancestors(t: &Arc<Type>) -> Vec<Arc<Type>> {
                let mut ancestors = vec![t.clone()];
                let mut t = t.clone();
                while let Type::Alias(AliasType { alias_type, .. }) = t.deref() {
                    let parent = alias_type.clone();
                    ancestors.push(parent.clone());
                    t = parent;
                }
                ancestors
            }
            let ancestors_of_a = get_ancestors(a);
            let ancestors_of_b = get_ancestors(b);
            // Traverse the ancestry of 'b' until we find a common ancestor with 'a'
            ancestors_of_b
                .into_iter()
                .find(|bi| ancestors_of_a.iter().any(|ai| Type::identical(ai, bi)))
        }

        // Apply this rule if either of t1 and t2 is an alias type
        if !t1.is_canonical() || !t2.is_canonical() {
            match lca(t1, t2) {
                // If there is a least common ancestor, then use it
                Some(c) => Some(c),
                // Otherwise use the common type of the underlying types
                None => Type::common_type(&Type::underlying_type(t1), &Type::underlying_type(t2)),
            }
        } else {
            None
        }
    }

    fn common_type_numeric(t1: &Arc<Type>, t2: &Arc<Type>) -> Option<Arc<Type>> {
        if (t1.is_float() && t2.is_numeric()) || (t1.is_numeric() && t2.is_float()) {
            Some(Arc::new(Type::Float(FloatKind::F64)))
        } else if t1.is_numeric() && t2.is_numeric() {
            Some(Arc::new(Type::Integer))
        } else {
            None
        }
    }

    fn common_type_string(t1: &Arc<Type>, t2: &Arc<Type>) -> Option<Arc<Type>> {
        match (t1.deref(), t2.deref()) {
            (Type::String(_), Type::String(_)) => Some(Arc::new(Type::String(None))),
            _ => None,
        }
    }

    fn common_type_enumeration(t1: &Arc<Type>, t2: &Arc<Type>) -> Option<Arc<Type>> {
        match (t1.deref(), t2.deref()) {
            (Type::Enum(ty), _) => {
                Type::common_type(&Arc::new(Type::PrimitiveInt(ty.rep_type)), t2)
            }
            (_, Type::Enum(ty)) => {
                Type::common_type(t1, &Arc::new(Type::PrimitiveInt(ty.rep_type)))
            }
            _ => None,
        }
    }

    fn common_type_array(t1: &Arc<Type>, t2: &Arc<Type>) -> Option<Arc<Type>> {
        /// Handle the case of a single anonymous array in either position
        fn single_anon_array(anon_array: &AnonArrayType, other: &Arc<Type>) -> Option<Arc<Type>> {
            if other.is_promotable_to_array() {
                let elt_type = Type::common_type(other, &anon_array.elt_type)?;
                Some(Arc::new(Type::AnonArray(AnonArrayType {
                    size: anon_array.size,
                    elt_type,
                })))
            } else {
                None
            }
        }
        match (t1.deref(), t2.deref()) {
            (_, Type::Array(array2)) => {
                Type::common_type(t1, &Arc::new(Type::AnonArray(array2.anon_array.clone())))
            }
            (Type::Array(array1), _) => {
                Type::common_type(&Arc::new(Type::AnonArray(array1.anon_array.clone())), t2)
            }
            (Type::AnonArray(anon_array1), Type::AnonArray(anon_array2)) => {
                if ArrayType::sizes_match(anon_array1.size, anon_array2.size) {
                    let size = ArrayType::common_size(anon_array1.size, anon_array2.size);
                    let elt_type = Type::common_type(&anon_array1.elt_type, &anon_array2.elt_type)?;
                    Some(Arc::new(Type::AnonArray(AnonArrayType { size, elt_type })))
                } else {
                    None
                }
            }
            (_, Type::AnonArray(anon_array)) => single_anon_array(anon_array, t1),
            (Type::AnonArray(anon_array), _) => single_anon_array(anon_array, t2),
            _ => None,
        }
    }

    fn common_type_struct(t1: &Arc<Type>, t2: &Arc<Type>) -> Option<Arc<Type>> {
        /// Handle the case of two anonymous structs
        fn two_anon_structs(
            anon_struct1: &AnonStructType,
            anon_struct2: &AnonStructType,
        ) -> Option<Arc<Type>> {
            // Resolve a member of t1 against the corresponding member of t2, if
            // it exists
            let resolve_t1_member = |member: &StructMember| {
                let (name1, ty1) = member;
                match anon_struct2.get_member(name1) {
                    Some(ty2) => Some((name1.clone(), Type::common_type(ty1, ty2)?)),
                    None => Some(member.clone()),
                }
            };
            // Resolve each member of t1 against the corresponding member of t2,
            // if it exists
            let t1_resolved_members =
                StructType::resolve_members(resolve_t1_member, &anon_struct1.members)?;
            let t2_resolved_members = anon_struct2
                .members
                .iter()
                .filter(|(name, _)| !anon_struct1.has_member(name))
                .cloned();
            Some(Arc::new(Type::AnonStruct(AnonStructType {
                members: t1_resolved_members
                    .into_iter()
                    .chain(t2_resolved_members)
                    .collect(),
            })))
        }
        /// Handle the case of a single anonymous struct in either position
        fn single_anon_struct(
            anon_struct: &AnonStructType,
            other: &Arc<Type>,
        ) -> Option<Arc<Type>> {
            if other.is_promotable_to_struct() {
                // Resolve a member of t2 against t1
                let resolve_member =
                    |(name, t): &StructMember| Some((name.clone(), Type::common_type(other, t)?));
                // Resolve all members of t2 against t1
                let resolved_members =
                    StructType::resolve_members(resolve_member, &anon_struct.members)?;
                Some(Arc::new(Type::AnonStruct(AnonStructType {
                    members: resolved_members,
                })))
            } else {
                None
            }
        }
        match (t1.deref(), t2.deref()) {
            (_, Type::Struct(struct2)) => {
                Type::common_type(t1, &Arc::new(Type::AnonStruct(struct2.anon_struct.clone())))
            }
            (Type::Struct(struct1), _) => {
                Type::common_type(&Arc::new(Type::AnonStruct(struct1.anon_struct.clone())), t2)
            }
            (Type::AnonStruct(anon_struct1), Type::AnonStruct(anon_struct2)) => {
                two_anon_structs(anon_struct1, anon_struct2)
            }
            (_, Type::AnonStruct(anon_struct)) => single_anon_struct(anon_struct, t1),
            (Type::AnonStruct(anon_struct), _) => single_anon_struct(anon_struct, t2),
            _ => None,
        }
    }
}

/// A rule of [`Type::common_type`]: the common type of a pair of types, if the
/// rule applies to them
type CommonTypeRule = fn(&Arc<Type>, &Arc<Type>) -> Option<Arc<Type>>;

/// Why a type is not convertible to another type. Each case is reported as a
/// note on the error that the failed conversion causes.
#[derive(Debug)]
pub enum TypeConversionError {
    /// The array sizes do not match
    ArraySizeMismatch { from: usize, to: usize },
    /// A single value promoted to an array does not convert to the element type
    ArrayElementDuringPromotion(Box<TypeConversionError>),
    /// The element type of an array does not convert
    ArrayElement(Box<TypeConversionError>),
    /// The type is not promotable to an array type
    NotPromotableToArray(Box<Type>),
    /// The type is not promotable to a struct type
    NotPromotableToStruct(Box<Type>),
    /// A member of the struct type converted from is not a member of the struct
    /// type converted to
    MissingStructMember(String),
    /// The type of a struct member does not convert
    StructMember {
        name: String,
        err: Box<TypeConversionError>,
    },
    /// The types are not convertible
    Mismatch { from: Box<Type>, to: Box<Type> },
}

impl TypeConversionError {
    /// Adds the notes explaining this conversion failure to a diagnostic
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
                diagnostic.note(format!("no member `{}` in the struct type", name))
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

/// Whether a type converts to another, and if not, why not
pub type TypeConversionResult = Result<(), TypeConversionError>;

/// Why [`Type::serialized_size`] could not produce a size
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SerializedSizeError {
    /// The size depends on information that is not there: a framework definition
    /// that is not defined, a type that is not finalized, or a type with no
    /// serialized representation at all (`Integer`, an abstract type).
    Unavailable,
    /// The size does not fit in an `i128`, the representation of a size, so it
    /// is reported rather than silently wrapped.
    TooLarge,
}

/// Computes the serialized size of a type
///
/// Only displayable types have a serialized size; any other type has the error
/// [`SerializedSizeError::Unavailable`]. Assumes that `FinalizeTypeDefs` has
/// been run on the analysis: the size of a string uses the framework
/// definitions `FwSizeStoreType` and `FW_FIXED_LENGTH_STRING_SIZE`, and the
/// size of an array or struct uses the sizes that `FinalizeTypeDefs` computes.
///
/// The size is a product over the nesting depth of the type, so it grows faster
/// than any single array size: 128 levels of `array A = [2] B` reach 2^128
/// bytes. Sizes are `i128` values, so a size that does not fit is reported as
/// [`SerializedSizeError::TooLarge`] rather than wrapped.
pub(crate) struct SerializedSize;

impl SerializedSize {
    /// `n * size`, or [`SerializedSizeError::TooLarge`]
    fn scale(n: usize, size: i128) -> Result<i128, SerializedSizeError> {
        (n as i128)
            .checked_mul(size)
            .ok_or(SerializedSizeError::TooLarge)
    }
}

impl TypeVisitor for SerializedSize {
    type In<'a> = &'a crate::Analysis;

    type Out = Result<i128, SerializedSizeError>;

    fn default(&self, _: &crate::Analysis, _: &Type) -> Self::Out {
        Err(SerializedSizeError::Unavailable)
    }

    fn alias_type(&self, a: &crate::Analysis, t: &AliasType) -> Self::Out {
        self.ty(a, &Type::underlying_type(&t.alias_type))
    }

    fn array(&self, a: &crate::Analysis, t: &Arc<ArrayType>) -> Self::Out {
        let n = t.anon_array.size.ok_or(SerializedSizeError::Unavailable)?;
        SerializedSize::scale(n, self.ty(a, &t.anon_array.elt_type)?)
    }

    fn boolean(&self, _: &crate::Analysis) -> Self::Out {
        Ok(1)
    }

    fn enumeration(&self, a: &crate::Analysis, t: &Arc<EnumType>) -> Self::Out {
        self.primitive_int(a, t.rep_type)
    }

    fn float(&self, _: &crate::Analysis, kind: FloatKind) -> Self::Out {
        match kind {
            FloatKind::F32 => Ok(4),
            FloatKind::F64 => Ok(8),
        }
    }

    fn primitive_int(&self, _: &crate::Analysis, kind: IntegerKind) -> Self::Out {
        match kind {
            IntegerKind::I8 | IntegerKind::U8 => Ok(1),
            IntegerKind::I16 | IntegerKind::U16 => Ok(2),
            IntegerKind::I32 | IntegerKind::U32 => Ok(4),
            IntegerKind::I64 | IntegerKind::U64 => Ok(8),
        }
    }

    fn string(&self, a: &crate::Analysis, size: Option<i128>) -> Self::Out {
        use SerializedSizeError::Unavailable;
        let length_size = {
            let fw_store_size_symbol = a
                .framework_definitions
                .type_map
                .get("FwSizeStoreType")
                .ok_or(Unavailable)?;
            let store_size_type = a
                .type_map
                .get(&fw_store_size_symbol.node())
                .ok_or(Unavailable)?;
            self.ty(a, store_size_type)?
        };
        let data_size = match size {
            Some(v) => v,
            None => {
                let default_string_size_symbol = a
                    .framework_definitions
                    .constant_map
                    .get("FW_FIXED_LENGTH_STRING_SIZE")
                    .ok_or(Unavailable)?;
                a.get_int_value(default_string_size_symbol.node())
                    .ok_or(Unavailable)?
            }
        };
        length_size
            .checked_add(data_size)
            .ok_or(SerializedSizeError::TooLarge)
    }

    fn struct_(&self, a: &crate::Analysis, t: &Arc<StructType>) -> Self::Out {
        let mut size = 0i128;
        for (m, t1) in &t.anon_struct.members {
            let member_size = self.ty(a, t1)?;
            let n = t.sizes.get(m).copied().unwrap_or(1);
            size = size
                .checked_add(SerializedSize::scale(n as usize, member_size)?)
                .ok_or(SerializedSizeError::TooLarge)?;
        }
        Ok(size)
    }
}
