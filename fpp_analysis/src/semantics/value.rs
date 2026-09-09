use crate::semantics::{AnonArrayType, AnonStructType, ArrayType, EnumType, StructType, Type};
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
                        let size = array_ty.anon_array.size?;
                        Some(Value::Array(ArrayValue {
                            anon_array: AnonArrayValue {
                                elements: std::iter::repeat_n(elt_value, size).collect(),
                            },
                            ty: ty.clone(),
                        }))
                    }
                    Type::AnonArray(array_ty) => {
                        let elt_value = self.convert(&array_ty.elt_type)?;
                        let size = array_ty.size?;
                        Some(Value::AnonArray(AnonArrayValue {
                            elements: std::iter::repeat_n(elt_value, size).collect(),
                        }))
                    }
                    Type::Struct(struct_ty) => {
                        let mut out_value = HashMap::default();
                        for (name, member_ty) in &struct_ty.anon_struct.members {
                            out_value.insert(name.clone(), self.clone().convert(member_ty)?);
                        }

                        Some(Value::Struct(StructValue {
                            anon_struct: AnonStructValue { members: out_value },
                            ty: ty.clone(),
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
                        value: from.value as i128,
                        kind: *to_kind,
                    }))
                }
                Type::Float(to_kind) => Some(Value::Float(FloatValue {
                    value: from.value,
                    kind: *to_kind,
                })),
                Type::Integer => Some(Value::Integer(IntegerValue(from.value as i128))),
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
            | Value::Struct(StructValue { ty: from_ty, .. })
            | Value::EnumConstant(EnumConstantValue { ty: from_ty, .. })
            | Value::AbsType(AbsTypeValue { ty: from_ty })
                if Type::identical(&ty, &Type::underlying_type(from_ty)) =>
            {
                Some(self.clone())
            }

            // Enum -> Integer
            Value::EnumConstant(value) => {
                let from_ty = value.ty();
                Value::PrimitiveInteger(PrimitiveIntegerValue {
                    value: value.value.1,
                    kind: from_ty.rep_type,
                })
                .convert(ty_a)
            }

            Value::AnonArray(anon_array) | Value::Array(ArrayValue { anon_array, .. }) => {
                let anon_array_ty = match ty.deref() {
                    Type::Array(ArrayType {
                        anon_array: anon_array_ty,
                        ..
                    })
                    | Type::AnonArray(anon_array_ty) => anon_array_ty,
                    _ => return None,
                };

                if let Some(n) = anon_array_ty.size
                    && n != anon_array.elements.len()
                {
                    return None;
                }

                let mut elements = Vec::with_capacity(anon_array.elements.len());
                for e in &anon_array.elements {
                    elements.push(e.convert(&anon_array_ty.elt_type)?);
                }

                match ty.deref() {
                    Type::Array(_) => Some(Value::Array(ArrayValue {
                        anon_array: AnonArrayValue { elements },
                        ty: ty.clone(),
                    })),
                    Type::AnonArray(_) => Some(Value::AnonArray(AnonArrayValue { elements })),
                    _ => None,
                }
            }

            Value::AnonStruct(anon_struct) | Value::Struct(StructValue { anon_struct, .. }) => {
                let mut members = HashMap::default();

                let to_ty = match ty.deref() {
                    // TODO(tumbar) default values need to come from struct type?
                    Type::Struct(StructType { anon_struct, .. }) => anon_struct,
                    Type::AnonStruct(anon_struct) => anon_struct,
                    _ => return None,
                };

                for (name, ty) in &to_ty.members {
                    let member_value = match anon_struct.members.get(name) {
                        // Use the default value
                        None => ty.default_value()?,
                        Some(member_value) => member_value.convert(ty)?,
                    };

                    members.insert(name.clone(), member_value);
                }

                match ty.deref() {
                    Type::Struct(_) => Some(Value::Struct(StructValue {
                        anon_struct: AnonStructValue { members },
                        ty: ty_a.clone(),
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
                    if enum_value.ty().rep_type == *kind_left {
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
                kind: value.ty().rep_type,
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
            |left, right| Ok(left + right),
        )
    }

    /// Divide one value by another
    pub fn div(&self, other: &Value) -> MathResult {
        self.binop(
            other,
            |left, right| {
                if *right == 0.0 {
                    Err(MathError::DivByZero)
                } else {
                    Ok(left / right)
                }
            },
            |left, right| {
                if *right == 0 {
                    Err(MathError::DivByZero)
                } else {
                    Ok(left / right)
                }
            },
        )
    }

    /// Multiply two values
    pub fn mul(&self, other: &Value) -> MathResult {
        self.binop(
            other,
            |left, right| Ok(left * right),
            |left, right| Ok(left * right),
        )
    }

    /// Subtract one value from another
    pub fn sub(&self, other: &Value) -> MathResult {
        self.binop(
            other,
            |left, right| Ok(left - right),
            |left, right| Ok(left - right),
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
            Value::EnumConstant(v) => v.ty.clone(),
            Value::AbsType(AbsTypeValue { ty }) => ty.clone(),
            Value::Array(ArrayValue { ty, .. }) => ty.clone(),
            Value::Struct(StructValue { ty, .. }) => ty.clone(),
            Value::AnonArray(AnonArrayValue { elements }) => {
                // Reads the element type from the first element (an anon-array
                // value is never empty in practice, since it is built from a
                // non-empty literal).
                let elt_type = elements
                    .first()
                    .map(|e| e.get_type())
                    .unwrap_or_else(|| Arc::new(Type::Integer));
                Arc::new(Type::AnonArray(AnonArrayType {
                    size: Some(elements.len()),
                    elt_type,
                }))
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

    /// Negates a value. Returns `None` for values that cannot be negated
    /// (strings, booleans, aggregates). Enums negate through their integer
    /// representation type.
    pub fn negate(&self) -> Option<Value> {
        match self {
            Value::PrimitiveInteger(PrimitiveIntegerValue { value, kind }) => {
                Some(Value::PrimitiveInteger(PrimitiveIntegerValue {
                    value: -value,
                    kind: *kind,
                }))
            }
            Value::Integer(IntegerValue(value)) => Some(Value::Integer(IntegerValue(-value))),
            Value::Float(FloatValue { value, kind }) => Some(Value::Float(FloatValue {
                value: -value,
                kind: *kind,
            })),
            Value::EnumConstant(v) => Value::PrimitiveInteger(PrimitiveIntegerValue {
                value: v.value.1,
                kind: v.ty().rep_type,
            })
            .negate(),
            _ => None,
        }
    }

    /// Left-shifts an integer value by another. Returns `None` unless both
    /// operands are integers or enums.
    pub fn shl(&self, other: &Value) -> Option<Value> {
        self.int_shift_op(other, |v, s| v << s)
    }

    /// Right-shifts an integer value by another. Returns `None` unless both
    /// operands are integers or enums.
    pub fn shr(&self, other: &Value) -> Option<Value> {
        self.int_shift_op(other, |v, s| v >> s)
    }

    /// Shared implementation for `<<`/`>>`. The left operand's kind is preserved
    /// (a `PrimitiveInt` stays that kind, an `Integer` stays `Integer`); enums
    /// are converted to their representation type first.
    fn int_shift_op(&self, other: &Value, op: impl Fn(i128, u32) -> i128) -> Option<Value> {
        let shift = u32::try_from(other.as_shift_int()?).ok()?;
        match self {
            Value::PrimitiveInteger(PrimitiveIntegerValue { value, kind }) => {
                Some(Value::PrimitiveInteger(PrimitiveIntegerValue {
                    value: op(*value, shift),
                    kind: *kind,
                }))
            }
            Value::Integer(IntegerValue(value)) => {
                Some(Value::Integer(IntegerValue(op(*value, shift))))
            }
            Value::EnumConstant(v) => Value::PrimitiveInteger(PrimitiveIntegerValue {
                value: v.value.1,
                kind: v.ty().rep_type,
            })
            .int_shift_op(other, op),
            _ => None,
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
            Value::AnonArray(AnonArrayValue { elements }) => Value::AnonArray(AnonArrayValue {
                elements: elements.iter().map(|e| e.truncate()).collect(),
            }),
            Value::Array(ArrayValue { anon_array, ty }) => Value::Array(ArrayValue {
                anon_array: AnonArrayValue {
                    elements: anon_array.elements.iter().map(|e| e.truncate()).collect(),
                },
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
            Value::EnumConstant(EnumConstantValue { value, .. }) => {
                let vi = value.1 as i32;
                f.write_fmt(format_args!("{vi}"))
            }
            Value::AnonArray(_) => f.write_str("[array value]"),
            Value::Array(_) => f.write_str("[array value]"),
            Value::AnonStruct(_) => f.write_str("{struct value}"),
            Value::Struct(_) => f.write_str("{struct value}"),
        }
    }
}

pub enum MathError {
    InvalidInputs,
    DivByZero,
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
    pub elements: Vec<Value>,
}

/// Array values
#[derive(Debug, Clone)]
pub struct ArrayValue {
    pub anon_array: AnonArrayValue,
    pub ty: Arc<Type>,
}

/// Enum constant values
#[derive(Debug, Clone)]
pub struct EnumConstantValue {
    pub value: (String, i128),
    pub ty: Arc<Type>,
}

impl EnumConstantValue {
    pub fn new(member_name: String, value: i128, ty: Arc<Type>) -> EnumConstantValue {
        match ty.deref() {
            Type::Enum(_) => (),
            _ => {
                panic!("expected enum type")
            }
        }

        EnumConstantValue {
            value: (member_name, value),
            ty,
        }
    }

    pub fn ty(&self) -> &EnumType {
        match self.ty.deref() {
            Type::Enum(e_ty) => e_ty,
            _ => {
                panic!("expected enum type")
            }
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
    pub ty: Arc<Type>,
}

/// An abstract type
#[derive(Debug, Clone)]
pub struct AbsTypeValue {
    pub ty: Arc<Type>,
}
