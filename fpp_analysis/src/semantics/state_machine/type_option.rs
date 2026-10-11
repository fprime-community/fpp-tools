use crate::semantics::{PrimitiveType, Type, int_kind_signedness};
use std::sync::Arc;

/// An FPP type option
pub struct TypeOption;

/// A type option: either a type or nothing
pub type TypeOptionT = Option<Arc<Type>>;

impl TypeOption {
    /// Shows a type option as a string
    pub fn show(to: &TypeOptionT) -> String {
        match to {
            Some(t) => t.to_string(),
            None => "None".to_string(),
        }
    }

    /// Computes the common type option of two type options
    pub fn common_type(to1: &TypeOptionT, to2: &TypeOptionT) -> Option<TypeOptionT> {
        match (to1, to2) {
            (Some(t1), Some(t2)) => {
                if Type::identical(t1, t2) {
                    Some(Some(t1.clone()))
                } else {
                    let u1 = Type::underlying_type(t1);
                    let u2 = Type::underlying_type(t2);
                    match (u1.as_ref(), u2.as_ref()) {
                        (Type::PrimitiveInt(int1), Type::PrimitiveInt(int2)) => {
                            if int_kind_signedness(*int1) != int_kind_signedness(*int2) {
                                None
                            } else if int2.bit_width() > int1.bit_width() {
                                Some(Some(u2.clone()))
                            } else {
                                Some(Some(u1.clone()))
                            }
                        }
                        (Type::Float(float1), Type::Float(float2)) => {
                            if float2.bit_width() > float1.bit_width() {
                                Some(Some(u2.clone()))
                            } else {
                                Some(Some(u1.clone()))
                            }
                        }
                        (Type::String(_), Type::String(_)) => {
                            Some(Some(Arc::new(Type::String(None))))
                        }
                        _ => None,
                    }
                }
            }
            _ => Some(None),
        }
    }

    /// Checks whether to1 is convertible to to2
    pub fn is_convertible_to(to1: &TypeOptionT, to2: &TypeOptionT) -> bool {
        match (to1, to2) {
            (Some(t1), Some(t2)) => {
                if Type::identical(t1, t2) {
                    true
                } else {
                    // Compare on the underlying types so that alias types are
                    // transparent (see `common_type`).
                    let u1 = Type::underlying_type(t1);
                    let u2 = Type::underlying_type(t2);
                    match (u1.as_ref(), u2.as_ref()) {
                        (Type::PrimitiveInt(int1), Type::PrimitiveInt(int2)) => {
                            (int_kind_signedness(*int1) == int_kind_signedness(*int2))
                                && (int2.bit_width() >= int1.bit_width())
                        }
                        (Type::Float(float1), Type::Float(float2)) => {
                            float2.bit_width() >= float1.bit_width()
                        }
                        (Type::String(_), Type::String(_)) => true,
                        _ => false,
                    }
                }
            }
            (_, None) => true,
            (None, _) => false,
        }
    }
}
