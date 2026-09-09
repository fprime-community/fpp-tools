use crate::semantics::Type;
use fpp_ast::{FloatKind, IntegerKind};
use std::sync::Arc;

/// An FPP type option
pub struct TypeOption;

/// A type option: either a type or nothing
pub type TypeOptionT = Option<Arc<Type>>;

fn int_signedness(kind: IntegerKind) -> bool {
    matches!(
        kind,
        IntegerKind::I8 | IntegerKind::I16 | IntegerKind::I32 | IntegerKind::I64
    )
}

fn int_bit_width(kind: IntegerKind) -> u32 {
    match kind {
        IntegerKind::U8 | IntegerKind::I8 => 8,
        IntegerKind::U16 | IntegerKind::I16 => 16,
        IntegerKind::U32 | IntegerKind::I32 => 32,
        IntegerKind::U64 | IntegerKind::I64 => 64,
    }
}

fn float_bit_width(kind: FloatKind) -> u32 {
    match kind {
        FloatKind::F32 => 32,
        FloatKind::F64 => 64,
    }
}

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
                            if int_signedness(*int1) != int_signedness(*int2) {
                                None
                            } else if int_bit_width(*int2) > int_bit_width(*int1) {
                                Some(Some(u2.clone()))
                            } else {
                                Some(Some(u1.clone()))
                            }
                        }
                        (Type::Float(float1), Type::Float(float2)) => {
                            if float_bit_width(*float2) > float_bit_width(*float1) {
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
                            (int_signedness(*int1) == int_signedness(*int2))
                                && (int_bit_width(*int2) >= int_bit_width(*int1))
                        }
                        (Type::Float(float1), Type::Float(float2)) => {
                            float_bit_width(*float2) >= float_bit_width(*float1)
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

/// Tests for the type-option rules used to type state-machine signals,
/// actions, and guards. These follow the FPP spec section "Type Options"
/// (docs/spec/Type-Options.adoc): "Conversion of Type Options" and
/// "Computing a Common Type Option".
#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantics::test_helpers::*;
    use fpp_ast::IntegerKind;

    // Convenience: wrap a type as Some(t)
    fn some(t: Arc<Type>) -> TypeOptionT {
        Some(t)
    }

    /// Structural equality on type options: `None`/`None` and `Some`/`Some`
    /// with structurally-equal types.
    fn option_eq(a: &TypeOptionT, b: &TypeOptionT) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(t1), Some(t2)) => types_structurally_eq(t1, t2),
            _ => false,
        }
    }

    // -----------------------------------------------------------------------
    // "common type option" should ...
    // -----------------------------------------------------------------------

    #[test]
    fn common_type_option_resolvable_pairs() {
        with_test_ctx(|| {
            use IntegerKind::*;
            // ((o1, o2), expected)
            let cases: Vec<((TypeOptionT, TypeOptionT), TypeOptionT)> = vec![
                // Rule 1: None is absorbing
                ((some(i32()), None), None),
                ((None, some(i32())), None),
                ((None, None), None),
                // Rule 2.1: identical types
                ((some(i32()), some(i32())), some(i32())),
                (
                    (some(default_enum()), some(default_enum())),
                    some(default_enum()),
                ),
                // Rules 2.3/2.4: same-signedness integers resolve to the wider type
                ((some(i8()), some(i32())), some(i32())),
                ((some(i32()), some(i8())), some(i32())),
                ((some(u16()), some(u64())), some(u64())),
                // Rule 2.5: floats resolve to the wider float
                ((some(f32()), some(f64())), some(f64())),
                ((some(f64()), some(f32())), some(f64())),
                // Rule 2.6: strings resolve to string
                ((some(string(None)), some(string(None))), some(string(None))),
                (
                    (some(string_with_size(8)), some(string_with_size(16))),
                    some(string(None)),
                ),
                // Rule 2.2: an alias is replaced with its underlying type, then
                // the rules reapply. alias(I16) vs I32 widens to I32 exactly as
                // bare I16 vs I32.
                ((some(alias_type("A", i16(), 10)), some(i32())), some(i32())),
                ((some(i32()), some(alias_type("A", i16(), 10))), some(i32())),
                // Alias whose underlying type is identical to the other operand
                ((some(alias_type("A", i32(), 11)), some(i32())), some(i32())),
                // Chained alias: A2 = A1 = I32
                (
                    (
                        some(alias_type("A2", alias_type("A1", i32(), 12), 13)),
                        some(i32()),
                    ),
                    some(i32()),
                ),
                // Two distinct aliases of the same underlying primitive
                (
                    (
                        some(alias_type("B1", i32(), 14)),
                        some(alias_type("B2", i32(), 15)),
                    ),
                    some(i32()),
                ),
                // Alias of a wider int vs a narrower alias, same signedness -> wider
                (
                    (
                        some(alias_type("W", i32(), 16)),
                        some(alias_type("N", i8(), 17)),
                    ),
                    some(i32()),
                ),
            ];
            let _ = U8;
            for ((o1, o2), expected) in cases {
                let got = TypeOption::common_type(&o1, &o2);
                match got {
                    Some(actual) => assert!(
                        option_eq(&actual, &expected),
                        "common_type({}, {}) expected {}, got {}",
                        TypeOption::show(&o1),
                        TypeOption::show(&o2),
                        TypeOption::show(&expected),
                        TypeOption::show(&actual),
                    ),
                    None => panic!(
                        "common_type({}, {}) expected {}, got mismatch (None)",
                        TypeOption::show(&o1),
                        TypeOption::show(&o2),
                        TypeOption::show(&expected),
                    ),
                }
            }
        });
    }

    #[test]
    fn common_type_option_unresolvable_pairs() {
        with_test_ctx(|| {
            use IntegerKind::*;
            let cases: Vec<(TypeOptionT, TypeOptionT)> = vec![
                // Mixed signedness has no rule
                (some(i32()), some(u32())),
                (some(alias_type("A", i32(), 20)), some(u32())),
                // Enum is NOT unwrapped to its representation type here (unlike
                // the general Type::common_type); enum vs. primitive is a mismatch.
                (some(enumeration("E", I32, 21)), some(i32())),
                (
                    some(alias_type("AE", enumeration("E", I32, 21), 22)),
                    some(i32()),
                ),
                // Numeric vs. string
                (some(i32()), some(string(None))),
                // Boolean vs. numeric
                (some(boolean()), some(i32())),
            ];
            let _ = U8;
            for (o1, o2) in cases {
                assert!(
                    TypeOption::common_type(&o1, &o2).is_none(),
                    "common_type({}, {}) expected mismatch (None), got a result",
                    TypeOption::show(&o1),
                    TypeOption::show(&o2),
                );
            }
        });
    }

    // -----------------------------------------------------------------------
    // "type option conversion" should ...
    // -----------------------------------------------------------------------

    #[test]
    fn type_option_convertible_pairs() {
        with_test_ctx(|| {
            let cases: Vec<(TypeOptionT, TypeOptionT)> = vec![
                // Any type option may be converted to None
                (some(i32()), None),
                (None, None),
                // Identical types
                (some(i32()), some(i32())),
                // Same-signedness widening
                (some(i8()), some(i32())),
                (some(u16()), some(u64())),
                // Float widening
                (some(f32()), some(f64())),
                // Strings
                (some(string(None)), some(string(None))),
                (some(string_with_size(8)), some(string(None))),
                // Rule 2.2: alias unwrapping. alias(I16) -> I32 (widen after unwrap).
                (some(alias_type("A", i16(), 30)), some(i32())),
                (some(i16()), some(alias_type("A", i32(), 31))),
                (some(alias_type("A", i32(), 32)), some(i32())),
            ];
            for (o1, o2) in cases {
                assert!(
                    TypeOption::is_convertible_to(&o1, &o2),
                    "expected {} -> {} to be convertible",
                    TypeOption::show(&o1),
                    TypeOption::show(&o2),
                );
            }
        });
    }

    #[test]
    fn type_option_inconvertible_pairs() {
        with_test_ctx(|| {
            use IntegerKind::*;
            let cases: Vec<(TypeOptionT, TypeOptionT)> = vec![
                // None -> Some is not allowed
                (None, some(i32())),
                // Narrowing
                (some(i32()), some(i16())),
                (some(i32()), some(alias_type("A", i16(), 33))),
                // Mixed signedness
                (some(i32()), some(u32())),
                // Float narrowing
                (some(f64()), some(f32())),
                // Enum is not unwrapped -> not convertible to a primitive here
                (some(enumeration("E", I32, 34)), some(i32())),
            ];
            let _ = U8;
            for (o1, o2) in cases {
                assert!(
                    !TypeOption::is_convertible_to(&o1, &o2),
                    "expected {} -> {} to NOT be convertible",
                    TypeOption::show(&o1),
                    TypeOption::show(&o2),
                );
            }
        });
    }
}
