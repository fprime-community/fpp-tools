use crate::semantics::{
    AbsType, AnonArrayType, AnonStructType, ArrayType, EnumType, StructType, Type,
};
use fpp_ast::{FloatKind, IntegerKind};
use rustc_hash::FxHashMap as HashMap;
use std::fmt;
use std::fmt::Formatter;
use std::ops::Deref;
use std::sync::Arc;

/// An FPP value
#[derive(Debug, Clone)]
pub enum Value {
    PrimitiveInteger(PrimitiveIntegerValue),
    AbsType(AbsTypeValue),
    Integer(IntegerValue),
    Float(FloatValue),
    Boolean(BooleanValue),
    String(StringValue),
    EnumConstant(EnumConstantValue),
    AnonArray(AnonArrayValue),
    Array(ArrayValue),
    AnonStruct(AnonStructValue),
    Struct(StructValue),
}

impl Value {
    fn is_promotable_to_aggregate(&self) -> bool {
        matches!(
            self,
            Value::PrimitiveInteger(_)
                | Value::Integer(_)
                | Value::Float(_)
                | Value::Boolean(_)
                | Value::String(_)
                | Value::EnumConstant(_)
        )
    }

    /// Convert this value to a type
    pub fn convert(&self, ty_a: &Arc<Type>) -> Option<Value> {
        match (self.convert_impl(ty_a), self.is_promotable_to_aggregate()) {
            (Some(value), _) => Some(value),
            (None, true) => {
                // Try to promote this single value an array/struct
                // (if that's what we are trying to convert to)
                let ty = Type::underlying_type(ty_a);
                match ty.deref() {
                    Type::Array(array_ty) => {
                        let elt_value = self.convert(&array_ty.anon_array.elt_type)?;
                        let anon_array = promote_to_anon_array(elt_value, array_ty.anon_array.size);
                        Some(Value::Array(ArrayValue {
                            anon_array,
                            ty: array_ty.clone(),
                        }))
                    }
                    Type::AnonArray(array_ty) => {
                        let elt_value = self.convert(&array_ty.elt_type)?;
                        Some(Value::AnonArray(promote_to_anon_array(
                            elt_value,
                            array_ty.size,
                        )))
                    }
                    Type::Struct(struct_ty) => {
                        let mut out_value = HashMap::default();
                        for (name, member_ty) in &struct_ty.anon_struct.members {
                            out_value.insert(name.clone(), self.clone().convert(member_ty)?);
                        }

                        Some(Value::Struct(StructValue {
                            anon_struct: AnonStructValue { members: out_value },
                            ty: struct_ty.clone(),
                        }))
                    }
                    Type::AnonStruct(struct_ty) => {
                        let mut out_value = HashMap::default();
                        for (name, member_ty) in &struct_ty.members {
                            out_value.insert(name.clone(), self.clone().convert(member_ty)?);
                        }

                        Some(Value::AnonStruct(AnonStructValue { members: out_value }))
                    }
                    _ => None,
                }
            }
            (None, false) => None,
        }
    }

    /// Convert this value to a distinct type
    fn convert_impl(&self, ty_a: &Arc<Type>) -> Option<Value> {
        let ty = Type::underlying_type(ty_a);

        match &self {
            Value::PrimitiveInteger(PrimitiveIntegerValue { value: from, .. })
            | Value::Integer(IntegerValue(from)) => match ty.deref() {
                Type::PrimitiveInt(to_kind) => {
                    Some(Value::PrimitiveInteger(PrimitiveIntegerValue {
                        value: *from,
                        kind: *to_kind,
                    }))
                }
                Type::Float(to_kind) => Some(Value::Float(FloatValue {
                    value: *from as f64,
                    kind: *to_kind,
                })),
                Type::Integer => Some(Value::Integer(IntegerValue(*from))),
                _ => None,
            },

            Value::Float(from) => match ty.deref() {
                Type::PrimitiveInt(to_kind) => {
                    Some(Value::PrimitiveInteger(PrimitiveIntegerValue {
                        value: float_to_int(from.value),
                        kind: *to_kind,
                    }))
                }
                Type::Float(to_kind) => Some(Value::Float(FloatValue {
                    value: from.value,
                    kind: *to_kind,
                })),
                Type::Integer => Some(Value::Integer(IntegerValue(float_to_int(from.value)))),
                _ => None,
            },

            Value::Boolean(BooleanValue(_)) => match ty.deref() {
                Type::Boolean => Some(self.clone()),
                _ => None,
            },

            Value::String(StringValue(_)) => match ty.deref() {
                Type::String(_) => Some(self.clone()),
                _ => None,
            },

            // Values that have a type to a definition
            // Check if they are the same as the type we are trying to convert to
            Value::Array(ArrayValue { ty: from_ty, .. })
                if ty.def_node_id() == Some(from_ty.node.node_id) =>
            {
                Some(self.clone())
            }
            Value::Struct(StructValue { ty: from_ty, .. })
                if ty.def_node_id() == Some(from_ty.node.node_id) =>
            {
                Some(self.clone())
            }
            Value::EnumConstant(EnumConstantValue { ty: from_ty, .. })
                if ty.def_node_id() == Some(from_ty.node.node_id) =>
            {
                Some(self.clone())
            }
            Value::AbsType(AbsTypeValue { ty: from_ty })
                if ty.def_node_id() == Some(from_ty.node.node_id) =>
            {
                Some(self.clone())
            }

            // Enum -> Integer
            Value::EnumConstant(value) => Value::PrimitiveInteger(PrimitiveIntegerValue {
                value: value.value.1,
                kind: value.ty.rep_type,
            })
            .convert(ty_a),

            Value::AnonArray(anon_array) | Value::Array(ArrayValue { anon_array, .. }) => {
                let anon_array_ty = ty.as_anon_array()?;

                if let Some(n) = anon_array_ty.size
                    && n != anon_array.elements.len()
                {
                    return None;
                }

                // The conversion is over the elements, so the result has no
                // scalar
                let elements =
                    anon_array.try_map_elements(|e| e.convert(&anon_array_ty.elt_type))?;

                match ty.deref() {
                    Type::Array(array_ty) => Some(Value::Array(ArrayValue {
                        anon_array: AnonArrayValue {
                            elements,
                            scalar: None,
                        },
                        ty: array_ty.clone(),
                    })),
                    Type::AnonArray(_) => Some(Value::AnonArray(AnonArrayValue {
                        elements,
                        scalar: None,
                    })),
                    _ => None,
                }
            }

            Value::AnonStruct(anon_struct) | Value::Struct(StructValue { anon_struct, .. }) => {
                let mut members = HashMap::default();

                let to_ty = ty.as_anon_struct()?;

                // The values to use for members that this value does not
                // provide. Converting to a named struct takes them from the
                // target type's own default value; converting to an anonymous
                // struct takes them from the source struct's type.
                let member_defaults: Option<&AnonStructValue> = match (ty.deref(), self) {
                    (Type::Struct(struct_ty), _) => {
                        struct_ty.default.as_ref().map(|d| &d.anon_struct)
                    }
                    (Type::AnonStruct(_), Value::Struct(StructValue { ty: from_ty, .. })) => {
                        from_ty.default.as_ref().map(|d| &d.anon_struct)
                    }
                    _ => None,
                };

                for (name, member_ty) in &to_ty.members {
                    let member_value = match anon_struct.members.get(name) {
                        Some(member_value) => member_value.convert(member_ty)?,
                        // The member is absent: take the struct type's default
                        // for it, or failing that the member type's default.
                        None => match member_defaults.and_then(|d| d.members.get(name)) {
                            Some(default) => default.clone(),
                            None => member_ty.default_value()?,
                        },
                    };

                    members.insert(name.clone(), member_value);
                }

                match ty.deref() {
                    Type::Struct(struct_ty) => Some(Value::Struct(StructValue {
                        anon_struct: AnonStructValue { members },
                        ty: struct_ty.clone(),
                    })),
                    Type::AnonStruct(_) => Some(Value::AnonStruct(AnonStructValue { members })),
                    _ => None,
                }
            }

            _ => None,
        }
    }

    /// Generic binary operation
    fn binop(
        &self,
        other: &Value,
        f64_op: fn(&f64, &f64) -> Result<f64, MathError>,
        i128_op: fn(&i128, &i128) -> Result<i128, MathError>,
    ) -> MathResult {
        match self {
            Value::PrimitiveInteger(PrimitiveIntegerValue {
                value: left,
                kind: kind_left,
            }) => match other {
                Value::PrimitiveInteger(PrimitiveIntegerValue {
                    value: right,
                    kind: kind_right,
                }) => {
                    if kind_left == kind_right {
                        Ok(Value::PrimitiveInteger(PrimitiveIntegerValue {
                            value: i128_op(left, right)?,
                            kind: *kind_left,
                        }))
                    } else {
                        Ok(Value::Integer(IntegerValue(i128_op(left, right)?)))
                    }
                }
                Value::Integer(IntegerValue(right)) => {
                    Ok(Value::Integer(IntegerValue(i128_op(left, right)?)))
                }
                Value::Float(FloatValue { value: right, .. }) => Ok(Value::Float(FloatValue {
                    value: f64_op(&(*left as f64), right)?,
                    kind: FloatKind::F64,
                })),
                Value::EnumConstant(
                    enum_value @ EnumConstantValue {
                        value: (_, right), ..
                    },
                ) => {
                    if enum_value.ty.rep_type == *kind_left {
                        Ok(Value::PrimitiveInteger(PrimitiveIntegerValue {
                            value: i128_op(left, right)?,
                            kind: *kind_left,
                        }))
                    } else {
                        Ok(Value::Integer(IntegerValue(i128_op(left, right)?)))
                    }
                }
                _ => Err(MathError::InvalidInputs),
            },

            Value::Integer(IntegerValue(left)) => match other {
                Value::Integer(IntegerValue(right))
                | Value::PrimitiveInteger(PrimitiveIntegerValue { value: right, .. })
                | Value::EnumConstant(EnumConstantValue {
                    value: (_, right), ..
                }) => Ok(Value::Integer(IntegerValue(i128_op(left, right)?))),
                Value::Float(FloatValue { value: right, .. }) => Ok(Value::Float(FloatValue {
                    value: f64_op(&(*left as f64), right)?,
                    kind: FloatKind::F64,
                })),
                _ => Err(MathError::InvalidInputs),
            },
            Value::Float(FloatValue {
                value: left,
                kind: left_kind,
            }) => match other {
                // Integral value + F64
                Value::Integer(IntegerValue(right))
                | Value::PrimitiveInteger(PrimitiveIntegerValue { value: right, .. })
                | Value::EnumConstant(EnumConstantValue {
                    value: (_, right), ..
                }) => Ok(Value::Float(FloatValue {
                    value: f64_op(left, &(*right as f64))?,
                    kind: FloatKind::F64,
                })),
                // Attempt to keep the same precision if we can
                Value::Float(FloatValue {
                    value: right,
                    kind: right_kind,
                }) => Ok(Value::Float(FloatValue {
                    value: f64_op(left, right)?,
                    kind: if left_kind == right_kind {
                        *left_kind
                    } else {
                        FloatKind::F64
                    },
                })),
                _ => Err(MathError::InvalidInputs),
            },
            Value::EnumConstant(value) => Value::PrimitiveInteger(PrimitiveIntegerValue {
                value: value.value.1,
                kind: value.ty.rep_type,
            })
            .binop(other, f64_op, i128_op),
            _ => Err(MathError::InvalidInputs),
        }
    }

    /// Add two values
    pub fn add(&self, other: &Value) -> MathResult {
        // String concatenation
        if let (Value::String(StringValue(left)), Value::String(StringValue(right))) = (self, other)
        {
            return Ok(Value::String(StringValue(format!("{left}{right}"))));
        }
        self.binop(
            other,
            |left, right| Ok(left + right),
            |left, right| left.checked_add(*right).ok_or(MathError::Overflow),
        )
    }

    /// Divide one value by another
    pub fn div(&self, other: &Value) -> MathResult {
        // The divisor is tested for zero before the operator is applied, so a
        // float divisor within `EPSILON` of zero is a division by zero even
        // though the raw `f64` division would succeed
        if other.is_zero() {
            return Err(MathError::DivByZero);
        }
        self.binop(
            other,
            |left, right| Ok(left / right),
            // `i128::MIN / -1` is the one integer division whose exact result is
            // out of range
            |left, right| left.checked_div(*right).ok_or(MathError::Overflow),
        )
    }

    /// Multiply two values
    pub fn mul(&self, other: &Value) -> MathResult {
        self.binop(
            other,
            |left, right| Ok(left * right),
            |left, right| left.checked_mul(*right).ok_or(MathError::Overflow),
        )
    }

    /// Subtract one value from another
    pub fn sub(&self, other: &Value) -> MathResult {
        self.binop(
            other,
            |left, right| Ok(left - right),
            |left, right| left.checked_sub(*right).ok_or(MathError::Overflow),
        )
    }

    /// Extract an integer value for use as a shift operand or shift amount.
    /// Returns `None` for non-integer values.
    pub fn as_shift_int(&self) -> Option<i128> {
        match self {
            Value::Integer(IntegerValue(v)) => Some(*v),
            Value::PrimitiveInteger(PrimitiveIntegerValue { value, .. }) => Some(*value),
            Value::EnumConstant(EnumConstantValue { value: (_, v), .. }) => Some(*v),
            _ => None,
        }
    }

    /// Gets the type of this value.
    pub fn get_type(&self) -> Arc<Type> {
        match self {
            Value::PrimitiveInteger(PrimitiveIntegerValue { kind, .. }) => {
                Arc::new(Type::PrimitiveInt(*kind))
            }
            Value::Integer(_) => Arc::new(Type::Integer),
            Value::Float(FloatValue { kind, .. }) => Arc::new(Type::Float(*kind)),
            Value::Boolean(_) => Arc::new(Type::Boolean),
            Value::String(_) => Arc::new(Type::String(None)),
            Value::EnumConstant(v) => Arc::new(Type::Enum(v.ty.clone())),
            Value::AbsType(AbsTypeValue { ty }) => Arc::new(Type::AbsType(ty.clone())),
            Value::Array(ArrayValue { ty, .. }) => Arc::new(Type::Array(ty.clone())),
            Value::Struct(StructValue { ty, .. }) => Arc::new(Type::Struct(ty.clone())),
            Value::AnonArray(AnonArrayValue { elements, scalar }) => {
                // The element type comes from the first element. A value
                // promoted to an array of unknown size has no elements, only a
                // scalar, and so has unknown size too.
                match (elements.first(), scalar) {
                    (Some(elt), _) => Arc::new(Type::AnonArray(AnonArrayType {
                        size: Some(elements.len()),
                        elt_type: elt.get_type(),
                    })),
                    (None, Some(scalar)) => Arc::new(Type::AnonArray(AnonArrayType {
                        size: None,
                        elt_type: scalar.get_type(),
                    })),
                    (None, None) => Arc::new(Type::AnonArray(AnonArrayType {
                        size: Some(0),
                        elt_type: Arc::new(Type::Integer),
                    })),
                }
            }
            Value::AnonStruct(AnonStructValue { members }) => {
                let member_types = members
                    .iter()
                    .map(|(name, v)| (name.clone(), v.get_type()))
                    .collect();
                Arc::new(Type::AnonStruct(AnonStructType {
                    members: member_types,
                }))
            }
        }
    }

    /// Whether this value is zero, for purposes of division. Floats use an
    /// epsilon comparison.
    pub fn is_zero(&self) -> bool {
        // Epsilon for nearness to zero
        const EPSILON: f64 = 0.0000001;
        match self {
            Value::PrimitiveInteger(PrimitiveIntegerValue { value, .. })
            | Value::Integer(IntegerValue(value)) => *value == 0,
            Value::Float(FloatValue { value, .. }) => value.abs() < EPSILON,
            Value::EnumConstant(EnumConstantValue { value: (_, v), .. }) => *v == 0,
            _ => false,
        }
    }

    /// Negates a value, preserving its kind. Reports
    /// [`MathError::InvalidInputs`] for values that cannot be negated (strings,
    /// booleans, aggregates) and [`MathError::Overflow`] for `i128::MIN`, whose
    /// negation is out of range. Enums negate through their integer
    /// representation type.
    pub fn negate(&self) -> MathResult {
        match self {
            Value::PrimitiveInteger(PrimitiveIntegerValue { value, kind }) => {
                Ok(Value::PrimitiveInteger(PrimitiveIntegerValue {
                    value: value.checked_neg().ok_or(MathError::Overflow)?,
                    kind: *kind,
                }))
            }
            Value::Integer(IntegerValue(value)) => Ok(Value::Integer(IntegerValue(
                value.checked_neg().ok_or(MathError::Overflow)?,
            ))),
            Value::Float(FloatValue { value, kind }) => Ok(Value::Float(FloatValue {
                value: -value,
                kind: *kind,
            })),
            Value::EnumConstant(v) => Value::PrimitiveInteger(PrimitiveIntegerValue {
                value: v.value.1,
                kind: v.ty.rep_type,
            })
            .negate(),
            _ => Err(MathError::InvalidInputs),
        }
    }

    /// Left-shift an integer value
    pub fn shl(&self, other: &Value) -> MathResult {
        self.int_shift_op(other, ShiftDirection::Left)
    }

    /// Right-shift an integer value
    pub fn shr(&self, other: &Value) -> MathResult {
        self.int_shift_op(other, ShiftDirection::Right)
    }

    /// Shift-only binary operation.
    ///
    /// The left operand determines the result: a `PrimitiveInteger` keeps its kind
    /// when the shift amount is also a `PrimitiveInteger` (or an enum constant,
    /// which is first converted to its representation type) but degrades to
    /// `Integer` when the amount is an `Integer`; an `Integer` always stays an
    /// `Integer`. Anything else is not shiftable.
    fn int_shift_op(&self, other: &Value, dir: ShiftDirection) -> MathResult {
        match self {
            Value::PrimitiveInteger(PrimitiveIntegerValue { value, kind }) => match other {
                Value::PrimitiveInteger(PrimitiveIntegerValue { value: amount, .. })
                | Value::EnumConstant(EnumConstantValue {
                    value: (_, amount), ..
                }) => Ok(Value::PrimitiveInteger(PrimitiveIntegerValue {
                    value: shift_int(*value, *amount, dir)?,
                    kind: *kind,
                })),
                Value::Integer(IntegerValue(amount)) => Ok(Value::Integer(IntegerValue(
                    shift_int(*value, *amount, dir)?,
                ))),
                _ => Err(MathError::InvalidInputs),
            },
            Value::Integer(IntegerValue(value)) => match other {
                Value::Integer(IntegerValue(amount))
                | Value::PrimitiveInteger(PrimitiveIntegerValue { value: amount, .. })
                | Value::EnumConstant(EnumConstantValue {
                    value: (_, amount), ..
                }) => Ok(Value::Integer(IntegerValue(shift_int(
                    *value, *amount, dir,
                )?))),
                _ => Err(MathError::InvalidInputs),
            },
            Value::EnumConstant(v) => Value::PrimitiveInteger(PrimitiveIntegerValue {
                value: v.value.1,
                kind: v.ty.rep_type,
            })
            .int_shift_op(other, dir),
            _ => Err(MathError::InvalidInputs),
        }
    }

    /// Truncates a value based on the width of its type: primitive integers wrap
    /// modulo their bit width, `F32` truncates to single precision, and
    /// aggregates truncate elementwise. Other values are unchanged.
    pub fn truncate(&self) -> Value {
        match self {
            Value::PrimitiveInteger(PrimitiveIntegerValue { value, kind }) => {
                Value::PrimitiveInteger(PrimitiveIntegerValue {
                    value: truncate_int(*value, *kind),
                    kind: *kind,
                })
            }
            Value::Float(FloatValue { value, kind }) => Value::Float(FloatValue {
                value: match kind {
                    FloatKind::F32 => *value as f32 as f64,
                    FloatKind::F64 => *value,
                },
                kind: *kind,
            }),
            Value::AnonArray(anon_array) => Value::AnonArray(anon_array.truncate()),
            Value::Array(ArrayValue { anon_array, ty }) => Value::Array(ArrayValue {
                anon_array: anon_array.truncate(),
                ty: ty.clone(),
            }),
            Value::AnonStruct(AnonStructValue { members }) => Value::AnonStruct(AnonStructValue {
                members: members
                    .iter()
                    .map(|(name, v)| (name.clone(), v.truncate()))
                    .collect(),
            }),
            Value::Struct(StructValue { anon_struct, ty }) => Value::Struct(StructValue {
                anon_struct: AnonStructValue {
                    members: anon_struct
                        .members
                        .iter()
                        .map(|(name, v)| (name.clone(), v.truncate()))
                        .collect(),
                },
                ty: ty.clone(),
            }),
            _ => self.clone(),
        }
    }
}

/// Truncates an integer to the width and signedness of `kind`, wrapping modulo
/// the type's range.
///
/// Wrapping is the operation itself here, not an accident of the `i128`
/// representation: every truncated value fits in an `i128`, so no result is
/// inexpressible.
fn truncate_int(value: i128, kind: IntegerKind) -> i128 {
    match kind {
        IntegerKind::I8 => value as i8 as i128,
        IntegerKind::I16 => value as i16 as i128,
        IntegerKind::I32 => value as i32 as i128,
        IntegerKind::I64 => value as i64 as i128,
        IntegerKind::U8 => value as u8 as i128,
        IntegerKind::U16 => value as u16 as i128,
        IntegerKind::U32 => value as u32 as i128,
        IntegerKind::U64 => value as u64 as i128,
    }
}

/// Narrows a float to an integer value, rounding towards zero.
///
/// The result is the float's integer part clamped to the range of an `i32`, with
/// a NaN going to zero, whatever the width of the target kind. The `i32` width
/// is part of the language's constant semantics, not an artifact: a subsequent
/// [`Value::truncate`] to the target kind reduces the retained bits modulo the
/// kind's width, so keeping more of the value would change the constant.
/// `array A = [1] I32 default [1.0e300]` is `2147483647`, but would be `-1` if
/// the conversion saturated at `i128::MAX` instead.
fn float_to_int(value: f64) -> i128 {
    // `as` on a float is a saturating cast in Rust, so this cannot trap however
    // large the float is
    value as i32 as i128
}

/// The direction of a shift operation
#[derive(Copy, Clone)]
enum ShiftDirection {
    Left,
    Right,
}

/// Shifts `value` by `amount` bits in the direction given by `dir`.
///
/// Right shifts are arithmetic, and an amount at or beyond the width of `i128`
/// saturates to `0` (non-negative values) or `-1` (negative values), which is what
/// an arbitrary-precision arithmetic shift by any larger amount produces. A left
/// shift whose mathematical result does not fit in `i128` reports
/// [`MathError::ShiftOverflow`].
fn shift_int(value: i128, amount: i128, dir: ShiftDirection) -> Result<i128, MathError> {
    // The constant evaluator rejects a negative shift amount before applying the
    // operator, so this guard is only reachable from direct callers of the
    // `Value` layer
    if amount < 0 {
        return Err(MathError::InvalidInputs);
    }
    match dir {
        ShiftDirection::Right => Ok(value >> (amount.min(127) as u32)),
        // Shifting zero is zero for any amount, however large
        ShiftDirection::Left if value == 0 => Ok(0),
        ShiftDirection::Left => {
            let amount = u32::try_from(amount).map_err(|_| MathError::ShiftOverflow)?;
            let shifted = value.checked_shl(amount).ok_or(MathError::ShiftOverflow)?;
            if (shifted >> amount) == value {
                Ok(shifted)
            } else {
                Err(MathError::ShiftOverflow)
            }
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Value::PrimitiveInteger(PrimitiveIntegerValue { value, .. }) => {
                f.write_fmt(format_args!("{value}"))
            }
            Value::AbsType(_) => f.write_str("[default value]"),
            Value::Integer(IntegerValue(value)) => f.write_fmt(format_args!("{value}")),
            Value::Float(FloatValue { value, .. }) => f.write_fmt(format_args!("{value}")),
            Value::Boolean(BooleanValue(value)) => f.write_fmt(format_args!("{value}")),
            Value::String(StringValue(value)) => f.write_fmt(format_args!("\"{value}\"")),
            Value::EnumConstant(EnumConstantValue {
                value: (_, value), ..
            }) => f.write_fmt(format_args!("{value}")),
            Value::AnonArray(_) => f.write_str("[array value]"),
            Value::Array(_) => f.write_str("[array value]"),
            Value::AnonStruct(_) => f.write_str("{struct value}"),
            Value::Struct(_) => f.write_str("{struct value}"),
        }
    }
}

#[derive(Debug)]
pub enum MathError {
    InvalidInputs,
    DivByZero,
    /// The mathematical result of an addition, subtraction, multiplication,
    /// division, or negation does not fit in an `i128`. Values are represented
    /// exactly here, so an inexpressible result is reported rather than
    /// silently wrapped.
    Overflow,
    /// The mathematical result of a left shift does not fit in an `i128`. Values
    /// are represented exactly here, so an inexpressible result is reported
    /// rather than silently wrapped.
    ShiftOverflow,
}

pub type MathResult = Result<Value, MathError>;

/// Primitive integer values
#[derive(Debug, Clone)]
pub struct PrimitiveIntegerValue {
    pub value: i128,
    pub kind: fpp_ast::IntegerKind,
}

/// Integer values
#[derive(Debug, Clone)]
pub struct IntegerValue(pub i128);

/// Floating-point values
#[derive(Debug, Clone)]
pub struct FloatValue {
    pub value: f64,
    pub kind: FloatKind,
}

/// Boolean values
#[derive(Debug, Clone)]
pub struct BooleanValue(pub bool);

/// String values
#[derive(Debug, Clone)]
pub struct StringValue(pub String);

/// Anonymous array values

#[derive(Debug, Clone)]
pub struct AnonArrayValue {
    /// The elements, in order. Use [`AnonArrayValue::iter`] to read them as
    /// `&Value`.
    pub elements: Vec<Arc<Value>>,
    /// The single element that this value was promoted from, when the array
    /// size was not known.
    pub scalar: Option<Arc<Value>>,
}

impl AnonArrayValue {
    /// An anonymous array value with the given elements
    pub fn new(elements: Vec<Value>) -> AnonArrayValue {
        AnonArrayValue {
            elements: elements.into_iter().map(Arc::new).collect(),
            scalar: None,
        }
    }

    /// An anonymous array value of `size` copies of one element
    ///
    /// The element is stored once, so this costs one pointer per element rather
    /// than one deep copy per element.
    #[allow(clippy::rc_clone_in_vec_init)]
    pub fn repeated(elt: Value, size: usize) -> AnonArrayValue {
        AnonArrayValue {
            elements: vec![Arc::new(elt); size],
            scalar: None,
        }
    }

    /// A single element promoted to an anonymous array value of unknown size:
    /// the element is recorded as the array's scalar and there are no elements
    pub fn promoted(elt: Value) -> AnonArrayValue {
        AnonArrayValue {
            elements: vec![],
            scalar: Some(Arc::new(elt)),
        }
    }

    /// The elements, as `&Value`
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Value> {
        self.elements.iter().map(|e| e.deref())
    }

    /// The element at `index`, if any
    pub fn get(&self, index: usize) -> Option<&Value> {
        self.elements.get(index).map(|e| e.deref())
    }

    /// Applies `f` to each element, preserving sharing: elements that are one
    /// shared value map to one shared result. A repeated element (an array
    /// default) is therefore mapped once, whatever the array's size. Returns
    /// `None` as soon as `f` does.
    fn try_map_elements(
        &self,
        mut f: impl FnMut(&Value) -> Option<Value>,
    ) -> Option<Vec<Arc<Value>>> {
        let mut out = Vec::with_capacity(self.elements.len());
        // The last element mapped, and its result. Repeated elements are all the
        // same `Arc`, so a single-entry cache is enough to keep them shared.
        let mut last: Option<(&Arc<Value>, Arc<Value>)> = None;
        for e in &self.elements {
            let shared = match &last {
                Some((prev, mapped)) if Arc::ptr_eq(prev, e) => Some(mapped.clone()),
                _ => None,
            };
            let mapped = match shared {
                Some(mapped) => mapped,
                None => {
                    let mapped = Arc::new(f(e)?);
                    last = Some((e, mapped.clone()));
                    mapped
                }
            };
            out.push(mapped);
        }

        Some(out)
    }

    /// Applies an infallible `f` to each element, preserving sharing
    fn map_elements(&self, mut f: impl FnMut(&Value) -> Value) -> Vec<Arc<Value>> {
        self.try_map_elements(|v| Some(f(v)))
            .expect("an infallible mapping cannot fail")
    }

    /// Truncates elementwise, preserving sharing
    fn truncate(&self) -> AnonArrayValue {
        AnonArrayValue {
            elements: self.map_elements(|e| e.truncate()),
            scalar: self.scalar.as_ref().map(|s| Arc::new(s.truncate())),
        }
    }
}

/// Array values
#[derive(Debug, Clone)]
pub struct ArrayValue {
    pub anon_array: AnonArrayValue,
    pub ty: Arc<ArrayType>,
}

/// Enum constant values
#[derive(Debug, Clone)]
pub struct EnumConstantValue {
    pub value: (String, i128),
    pub ty: Arc<EnumType>,
}

impl EnumConstantValue {
    pub fn new(member_name: String, value: i128, ty: Arc<EnumType>) -> EnumConstantValue {
        EnumConstantValue {
            value: (member_name, value),
            ty,
        }
    }
}

/// Anonymous struct values
#[derive(Debug, Clone)]
pub struct AnonStructValue {
    pub members: HashMap<String, Value>,
}

/// Struct values
#[derive(Debug, Clone)]
pub struct StructValue {
    pub anon_struct: AnonStructValue,
    pub ty: Arc<StructType>,
}

/// An abstract type
#[derive(Debug, Clone)]
pub struct AbsTypeValue {
    pub ty: Arc<AbsType>,
}

/// Promotes a single element value to an anonymous array value. If the array
/// size is not known, the element is recorded as the array's scalar instead.
fn promote_to_anon_array(elt: Value, size: Option<usize>) -> AnonArrayValue {
    match size {
        Some(size) => AnonArrayValue::repeated(elt, size),
        None => AnonArrayValue::promoted(elt),
    }
}
