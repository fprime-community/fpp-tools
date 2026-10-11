#![cfg(test)]

//! Tests for the type-option rules used to type state-machine signals,
//! actions, and guards. These follow the FPP spec section "Type Options"
//! (docs/spec/Type-Options.adoc): "Conversion of Type Options" and
//! "Computing a Common Type Option".

use super::spec::*;
use crate::semantics::Type;
use crate::semantics::state_machine::{TypeOption, TypeOptionT};
use std::sync::Arc;

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

// ---------------------------------------------------------------------------
// "common type option" should ...
// ---------------------------------------------------------------------------

#[test]
fn common_type_option_resolvable_pairs() {
    with_test_ctx(|| {
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
        use fpp_ast::IntegerKind::I32;
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

// ---------------------------------------------------------------------------
// "type option conversion" should ...
// ---------------------------------------------------------------------------

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
        use fpp_ast::IntegerKind::I32;
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
