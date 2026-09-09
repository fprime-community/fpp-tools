#![cfg(test)]

use super::test_helpers::*;
use crate::semantics::Type;

#[test]
fn node_id_memo_identity() {
    with_test_ctx(|| {
        let e1 = enumeration("E", fpp_ast::IntegerKind::I32, 0);
        let e2 = enumeration("E", fpp_ast::IntegerKind::I32, 0);
        assert!(Type::identical(&e1, &e2), "same id -> identical");

        let e3 = enumeration("E", fpp_ast::IntegerKind::I32, 1);
        assert!(!Type::identical(&e1, &e3), "different id -> not identical");
    });
}

// ---------------------------------------------------------------------------
// "type identity" should ...
// ---------------------------------------------------------------------------

#[test]
fn type_identity_holds_for_identical_pairs() {
    with_test_ctx(|| {
        // duplicate(x) -> (x, x); each must be identical to itself.
        let cases: Vec<(std::sync::Arc<Type>, std::sync::Arc<Type>)> = vec![
            (default_abs_type(), default_abs_type()),
            (default_array(), default_array()),
            (default_enum(), default_enum()),
            (default_struct(), default_struct()),
            (default_alias_type(), default_alias_type()),
            (boolean(), boolean()),
            (f32(), f32()),
            (f64(), f64()),
            (i16(), i16()),
            (i32(), i32()),
            (i64(), i64()),
            (i8(), i8()),
            (integer(), integer()),
            (u16(), u16()),
            (u32(), u32()),
            (u64(), u64()),
            (u8(), u8()),
        ];
        for (a, b) in cases {
            assert!(Type::identical(&a, &b), "expected identical: {a} == {b}");
        }
    });
}

#[test]
fn type_identity_does_not_hold_for_distinct_pairs() {
    with_test_ctx(|| {
        use fpp_ast::IntegerKind::*;
        let cases: Vec<(std::sync::Arc<Type>, std::sync::Arc<Type>)> = vec![
            (abs_type("T0", 0), abs_type("T1", 1)),
            (enumeration("E0", I32, 0), enumeration("E1", U32, 1)),
            (
                array("A0", anon_array(None, i32()), 0),
                array("A1", anon_array(None, u32()), 1),
            ),
            (
                struct_ty("S0", anon_struct(&[]), 0),
                struct_ty("S1", anon_struct(&[]), 1),
            ),
            (
                array("A", anon_array(None, i32()), 0),
                struct_ty("S0", anon_struct(&[]), 1),
            ),
            (default_abs_type(), default_enum()),
            (default_array(), default_struct()),
            (boolean(), string(None)),
            // duplicate(String(None)) -> two unsized strings are NOT identical:
            // identical() has no String arm, so they fall through to the
            // def_node_id comparison, which strings lack.
            (string(None), string(None)),
            (f32(), f64()),
            (i8(), u32()),
            // duplicate(AnonArray(None, I32)) -> anon arrays are not identical
            (anon_array(None, i32()), anon_array(None, i32())),
            // duplicate(AnonStruct(Map())) -> anon structs are not identical
            (anon_struct(&[]), anon_struct(&[])),
            (alias_type("TAliasU32", u32(), 0), u32()),
            (
                alias_type("AliasE", enumeration("E", I32, 0), 1),
                enumeration("E", I32, 0),
            ),
        ];
        for (a, b) in cases {
            assert!(
                !Type::identical(&a, &b),
                "expected NOT identical: {a} vs {b}"
            );
        }
    });
}

// ---------------------------------------------------------------------------
// "type conversion" should ...
// ---------------------------------------------------------------------------

fn may_be_converted(from: &std::sync::Arc<Type>, to: &std::sync::Arc<Type>) -> bool {
    Type::convert(from, to).is_ok()
}

#[test]
fn type_conversion_allowed_pairs() {
    with_test_ctx(|| {
        use fpp_ast::IntegerKind::*;
        // (from, to) pairs that must be convertible.
        let cases: Vec<(std::sync::Arc<Type>, std::sync::Arc<Type>)> = vec![
            (i32(), i32()),
            (i32(), f32()),
            (enumeration("E", I32, 0), i32()),
            (anon_array(None, i32()), anon_array(None, i32())),
            (anon_array(Some(3), i32()), anon_array(Some(3), i32())),
            (
                anon_array(Some(3), i32()),
                anon_array(Some(3), anon_array(None, u32())),
            ),
            (anon_array(None, i32()), anon_array(Some(3), i32())),
            (anon_array(Some(3), i32()), anon_array(None, i32())),
            (
                anon_array(None, enumeration("E", I32, 0)),
                anon_array(None, i32()),
            ),
            (
                array("A", anon_array(None, i32()), 0),
                anon_array(None, u32()),
            ),
            (
                array("A0", anon_array(None, i32()), 0),
                array("A1", anon_array(None, u32()), 1),
            ),
            (string(None), anon_array(None, string(None))),
            (enumeration("E", I32, 0), anon_array(None, i32())),
            (
                anon_array(Some(3), i32()),
                anon_array(Some(3), anon_array(Some(3), i32())),
            ),
            (anon_struct(&[("x", i32())]), anon_struct(&[("x", i32())])),
            (
                anon_struct(&[("x", i32())]),
                anon_struct(&[("x", anon_array(None, i32()))]),
            ),
            (
                anon_struct(&[("x", i32())]),
                anon_struct(&[("x", i32()), ("y", i32())]),
            ),
            (
                struct_ty("S", anon_struct(&[("x", i32())]), 0),
                anon_struct(&[("x", i32())]),
            ),
            (
                struct_ty("S0", anon_struct(&[("x", i32())]), 0),
                struct_ty("S1", anon_struct(&[("x", i32())]), 1),
            ),
            (string(None), anon_struct(&[("x", string(None))])),
            (
                anon_struct(&[("x", i32())]),
                anon_struct(&[("x", anon_struct(&[("y", i32())]))]),
            ),
            (default_alias_type(), default_alias_type()),
            (
                enumeration("E", I32, 0),
                alias_type("AliasE", enumeration("E", I32, 0), 1),
            ),
            (
                alias_type("AliasE", enumeration("E", I32, 0), 1),
                enumeration("E", I32, 0),
            ),
            (alias_type("AliasE", enumeration("E", I32, 0), 1), i32()),
            (default_alias_type(), default_abs_type()),
            (alias_type("AliasU32", u32(), 0), u32()),
            (u32(), alias_type("AliasU32", u32(), 0)),
        ];
        for (from, to) in cases {
            assert!(
                may_be_converted(&from, &to),
                "expected convertible: {from} -> {to}"
            );
        }
    });
}

#[test]
fn type_conversion_disallowed_pairs() {
    with_test_ctx(|| {
        use fpp_ast::IntegerKind::*;
        let cases: Vec<(std::sync::Arc<Type>, std::sync::Arc<Type>)> = vec![
            (string(None), boolean()),
            (i32(), enumeration("E", I32, 0)),
            (
                anon_array(None, i32()),
                anon_array(None, enumeration("E", I32, 0)),
            ),
            (anon_array(Some(3), i32()), anon_array(Some(4), i32())),
            (
                array("A", anon_array(None, i32()), 0),
                anon_array(None, string(None)),
            ),
            (
                array("A0", anon_array(None, i32()), 0),
                array("A1", anon_array(None, string(None)), 1),
            ),
            (string(None), anon_array(None, i32())),
            (
                anon_struct(&[("x", i32())]),
                anon_struct(&[("x", string(None))]),
            ),
            (anon_struct(&[("x", i32())]), anon_struct(&[("y", i32())])),
            (
                struct_ty("S", anon_struct(&[("x", i32())]), 0),
                anon_struct(&[("x", string(None))]),
            ),
            (
                struct_ty("S0", anon_struct(&[("x", i32())]), 0),
                struct_ty("S1", anon_struct(&[("x", string(None))]), 1),
            ),
            (string(None), anon_struct(&[("x", i32())])),
            (anon_array(None, i32()), anon_struct(&[])),
            (anon_struct(&[]), anon_array(None, i32())),
            (i32(), alias_type("AliasE", enumeration("E", I32, 0), 1)),
            (
                struct_ty("S", anon_struct(&[("x", i32())]), 0),
                alias_type("U32Alias", enumeration("E", U16, 1), 2),
            ),
        ];
        for (from, to) in cases {
            assert!(
                !may_be_converted(&from, &to),
                "expected NOT convertible: {from} -> {to}"
            );
        }
    });
}

// ---------------------------------------------------------------------------
// "common types" should ...
// ---------------------------------------------------------------------------

#[test]
fn common_types_allowed_pairs() {
    with_test_ctx(|| {
        use fpp_ast::IntegerKind::*;
        // ((t1, t2), expected)
        type P = (std::sync::Arc<Type>, std::sync::Arc<Type>);
        let cases: Vec<(P, std::sync::Arc<Type>)> = vec![
            ((i32(), i32()), i32()),
            ((default_abs_type(), default_abs_type()), default_abs_type()),
            ((default_array(), default_array()), default_array()),
            ((default_enum(), default_enum()), default_enum()),
            ((default_struct(), default_struct()), default_struct()),
            ((i32(), i8()), integer()),
            ((f32(), f64()), f64()),
            ((i32(), f64()), f64()),
            ((f64(), i32()), f64()),
            // Enums
            ((enumeration("E", I32, 0), i32()), i32()),
            ((enumeration("E", I32, 0), f64()), f64()),
            ((i32(), enumeration("E", I32, 0)), i32()),
            ((f64(), enumeration("E", I32, 0)), f64()),
            (
                (enumeration("E0", I32, 0), enumeration("E1", I32, 1)),
                i32(),
            ),
            (
                (enumeration("E0", I32, 0), enumeration("E1", I8, 1)),
                integer(),
            ),
            // Arrays
            (
                (anon_array(Some(3), i32()), anon_array(Some(3), i8())),
                anon_array(Some(3), integer()),
            ),
            (
                (integer(), anon_array(Some(3), integer())),
                anon_array(Some(3), integer()),
            ),
            (
                (anon_array(Some(3), integer()), integer()),
                anon_array(Some(3), integer()),
            ),
            (
                (
                    anon_array(Some(3), integer()),
                    anon_array(Some(3), anon_array(Some(4), integer())),
                ),
                anon_array(Some(3), anon_array(Some(4), integer())),
            ),
            (
                (
                    array("A0", anon_array(Some(3), i32()), 0),
                    array("A1", anon_array(Some(3), i8()), 1),
                ),
                anon_array(Some(3), integer()),
            ),
            (
                (
                    anon_array(Some(3), i32()),
                    array("A1", anon_array(Some(3), i8()), 1),
                ),
                anon_array(Some(3), integer()),
            ),
            (
                (
                    array("A0", anon_array(Some(3), i32()), 0),
                    anon_array(Some(3), i8()),
                ),
                anon_array(Some(3), integer()),
            ),
            // Structs
            (
                (anon_struct(&[("x", i32())]), anon_struct(&[("x", i32())])),
                anon_struct(&[("x", i32())]),
            ),
            (
                (anon_struct(&[("x", i32())]), anon_struct(&[("x", i8())])),
                anon_struct(&[("x", integer())]),
            ),
            (
                (anon_struct(&[("x", i32())]), anon_struct(&[("y", i8())])),
                anon_struct(&[("x", i32()), ("y", i8())]),
            ),
            (
                (
                    anon_struct(&[("x", i32())]),
                    anon_struct(&[("x", i8()), ("y", i8())]),
                ),
                anon_struct(&[("x", integer()), ("y", i8())]),
            ),
            (
                (integer(), anon_struct(&[("x", integer())])),
                anon_struct(&[("x", integer())]),
            ),
            (
                (anon_struct(&[("x", integer())]), integer()),
                anon_struct(&[("x", integer())]),
            ),
            (
                (
                    anon_struct(&[("x", i32())]),
                    anon_struct(&[("x", anon_struct(&[("x", i8())]))]),
                ),
                anon_struct(&[("x", anon_struct(&[("x", integer())]))]),
            ),
            (
                (
                    struct_ty("S0", anon_struct(&[("x", i32())]), 0),
                    struct_ty("A1", anon_struct(&[("x", i8())]), 1),
                ),
                anon_struct(&[("x", integer())]),
            ),
            (
                (
                    anon_struct(&[("x", i32())]),
                    struct_ty("A1", anon_struct(&[("x", i8())]), 1),
                ),
                anon_struct(&[("x", integer())]),
            ),
            (
                (
                    struct_ty("S0", anon_struct(&[("x", i32())]), 0),
                    anon_struct(&[("x", i8())]),
                ),
                anon_struct(&[("x", integer())]),
            ),
            // (enum E, type E1 = E) -> E
            (
                (
                    enumeration("E", I32, 0),
                    alias_type("E1", enumeration("E", I32, 0), 1),
                ),
                enumeration("E", I32, 0),
            ),
            // Simple common underlying type
            (
                (
                    alias_type("E1", enumeration("E", I32, 0), 1),
                    alias_type("E2", enumeration("E", I32, 0), 2),
                ),
                enumeration("E", I32, 0),
            ),
            // Common ancestor #1
            (
                (
                    alias_type("E3", alias_type("E1", enumeration("E", I32, 0), 2), 4),
                    alias_type("E2", alias_type("E1", enumeration("E", I32, 0), 2), 5),
                ),
                alias_type("E1", enumeration("E", I32, 0), 2),
            ),
            // Common ancestor #2
            (
                (
                    alias_type("E2", alias_type("E1", enumeration("E", I32, 0), 2), 5),
                    alias_type(
                        "E4",
                        alias_type("E3", alias_type("E1", enumeration("E", I32, 0), 2), 4),
                        6,
                    ),
                ),
                alias_type("E1", enumeration("E", I32, 0), 2),
            ),
            // No common ancestor -> compute using the underlying type
            (
                (
                    alias_type("A1", enumeration("E0", I32, 0), 3),
                    alias_type("A2", enumeration("E1", I32, 1), 4),
                ),
                i32(),
            ),
        ];
        for ((t1, t2), expected) in cases {
            let ct = Type::common_type(&t1, &t2);
            assert_common_type_eq(&ct, &expected);
        }
    });
}

#[test]
fn common_types_disallowed_pairs() {
    with_test_ctx(|| {
        type P = (std::sync::Arc<Type>, std::sync::Arc<Type>);
        let cases: Vec<P> = vec![
            (string(None), boolean()),
            (anon_array(None, i32()), anon_struct(&[])),
            (anon_struct(&[]), anon_array(None, i32())),
            (anon_array(Some(3), i32()), anon_array(Some(4), i8())),
            (
                anon_array(Some(3), i32()),
                anon_array(Some(3), string(None)),
            ),
            (string(None), anon_array(Some(3), integer())),
            (anon_array(Some(3), integer()), string(None)),
            (
                anon_array(Some(4), integer()),
                anon_array(Some(3), anon_array(Some(4), integer())),
            ),
            (
                anon_struct(&[("x", integer())]),
                anon_struct(&[("x", string(None))]),
            ),
            (string(None), anon_struct(&[("x", integer())])),
            (anon_struct(&[("x", integer())]), string(None)),
            (default_array(), default_struct()),
            (default_struct(), default_array()),
            (default_array(), anon_struct(&[])),
            (anon_struct(&[]), default_array()),
            (default_struct(), anon_array(None, i32())),
            (anon_array(None, i32()), default_struct()),
        ];
        for (t1, t2) in cases {
            let ct = Type::common_type(&t1, &t2);
            assert!(
                ct.is_none(),
                "expected no common type for ({t1}, {t2}), got {}",
                ct.map(|t| t.to_string()).unwrap_or_default()
            );
        }
    });
}

// ---------------------------------------------------------------------------
// "displayable type" should ...
// ---------------------------------------------------------------------------

#[test]
fn displayable_types() {
    with_test_ctx(|| {
        use fpp_ast::IntegerKind::*;
        let displayable: Vec<std::sync::Arc<Type>> = vec![
            i32(),
            f32(),
            boolean(),
            string(None),
            default_enum(),
            default_array(),
            default_struct(),
            array(
                "A1",
                anon_array(None, array("A2", anon_array(None, i32()), 10)),
                11,
            ),
            struct_ty("S1", anon_struct(&[("x", i32())]), 12),
        ];
        for ty in displayable {
            assert!(ty.is_displayable(), "expected displayable: {ty}");
        }

        let not_displayable: Vec<std::sync::Arc<Type>> = vec![
            integer(),
            anon_array(None, i32()),
            anon_struct(&[]),
            default_abs_type(),
            array("A3", anon_array(None, default_abs_type()), 20),
            array(
                "A4",
                anon_array(None, array("A5", anon_array(None, default_abs_type()), 21)),
                22,
            ),
            struct_ty("S2", anon_struct(&[("x", default_abs_type())]), 23),
            struct_ty(
                "S3",
                anon_struct(&[(
                    "x",
                    struct_ty("S4", anon_struct(&[("x", default_abs_type())]), 24),
                )]),
                25,
            ),
        ];
        let _ = U8;
        for ty in not_displayable {
            assert!(!ty.is_displayable(), "expected NOT displayable: {ty}");
        }
    });
}

// ---------------------------------------------------------------------------
// "size of" should ...
//
// String data size default = FW_FIXED_LENGTH_STRING_SIZE = 256; the length
// prefix type FwSizeStoreType = U16 contributes 2 bytes.
// ---------------------------------------------------------------------------

#[test]
fn size_of_primitives() {
    with_test_ctx(|| {
        let a = sizeof_test_analysis();
        let cases: Vec<(std::sync::Arc<Type>, i128)> = vec![
            (u8(), 1),
            (u16(), 2),
            (u32(), 4),
            (u64(), 8),
            (i8(), 1),
            (i16(), 2),
            (i32(), 4),
            (i64(), 8),
            (f32(), 4),
            (f64(), 8),
            (boolean(), 1),
        ];
        for (ty, expected) in cases {
            assert_eq!(ty.serialized_size(&a), Some(expected), "size of {ty}");
        }
    });
}

#[test]
fn size_of_alias_types() {
    with_test_ctx(|| {
        use fpp_ast::IntegerKind::U16;
        let a = sizeof_test_analysis();

        // StringAlias = alias of (string size 100) -> 2 + 100 = 102
        let string_alias = alias_type("StringAlias", string_with_size(100), 104);
        assert_eq!(string_alias.serialized_size(&a), Some(102));

        // alias of unsized string -> 2 + 256 = 258
        let string_default_alias = alias_type("StringDefaultAlias", string(None), 105);
        assert_eq!(string_default_alias.serialized_size(&a), Some(258));

        // EnumAlias = alias of (enum rep U16) -> 2
        let enum1 = enumeration("E", U16, 100);
        let enum_alias = alias_type("EnumAlias", enum1, 101);
        assert_eq!(enum_alias.serialized_size(&a), Some(2));
    });
}

#[test]
fn size_of_arrays() {
    with_test_ctx(|| {
        use fpp_ast::IntegerKind::U16;
        let a = sizeof_test_analysis();
        let _ = U16;
        // array1 = [3] I64 -> 3 * 8 = 24
        let array1 = array("A", anon_array(Some(3), i64()), 102);
        assert_eq!(array1.serialized_size(&a), Some(24));
        // array2 = [2] array1 -> 2 * 24 = 48
        let array2 = array("A2", anon_array(Some(2), array1), 103);
        assert_eq!(array2.serialized_size(&a), Some(48));
    });
}

#[test]
fn size_of_enums() {
    with_test_ctx(|| {
        use fpp_ast::IntegerKind::U16;
        let a = sizeof_test_analysis();
        let enum1 = enumeration("E", U16, 100);
        assert_eq!(enum1.serialized_size(&a), Some(2));
    });
}

#[test]
fn size_of_structs() {
    with_test_ctx(|| {
        use fpp_ast::IntegerKind::U16;
        let a = sizeof_test_analysis();

        let array1 = array("A", anon_array(Some(3), i64()), 102); // 24
        let array2 = array("A2", anon_array(Some(2), array1.clone()), 103); // 48
        let enum1 = enumeration("E", U16, 100);
        let enum_alias = alias_type("EnumAlias", enum1, 101); // 2
        let string_size_10 = string_with_size(10); // 12
        let string_alias = alias_type("StringAlias", string_with_size(100), 104); // 102

        // struct1: m1=array1(24)*2=48, m2=F64(8)=8, m3=enum_alias(2)=2, m4=string10(12)*3=36 => 94
        let struct1 = struct_ty_sized(
            "S",
            anon_struct(&[
                ("m1", array1.clone()),
                ("m2", f64()),
                ("m3", enum_alias.clone()),
                ("m4", string_size_10.clone()),
            ]),
            110,
            &[("m1", 2), ("m3", 1), ("m4", 3)],
        );
        assert_eq!(struct1.serialized_size(&a), Some(94));

        // struct2: m1=array2(48)=48, m2=string_alias(102)=102, m3=struct1(94)*2=188 => 338
        let struct2 = struct_ty_sized(
            "S2",
            anon_struct(&[
                ("m1", array2.clone()),
                ("m2", string_alias.clone()),
                ("m3", struct1.clone()),
            ]),
            111,
            &[("m3", 2)],
        );
        assert_eq!(struct2.serialized_size(&a), Some(338));
    });
}

#[test]
fn size_of_types_with_no_size() {
    with_test_ctx(|| {
        let a = sizeof_test_analysis();
        assert_eq!(default_abs_type().serialized_size(&a), None);

        let arr = array("A", anon_array(Some(3), default_abs_type()), 120);
        assert_eq!(arr.serialized_size(&a), None);

        let s = struct_ty(
            "S",
            anon_struct(&[("m1", i32()), ("m2", default_abs_type())]),
            121,
        );
        assert_eq!(s.serialized_size(&a), None);

        let x = alias_type("X", default_abs_type(), 122);
        assert_eq!(x.serialized_size(&a), None);
    });
}

// ---------------------------------------------------------------------------
// Additional coverage: `Type` predicate/helper methods (many recurse through
// alias types), exercising the same `Type` surface.
// ---------------------------------------------------------------------------

use crate::semantics::Value;

#[test]
fn type_predicates_and_helpers() {
    with_test_ctx(|| {
        let alias_i32 = alias_type("AI", i32(), 600);
        let alias_f32 = alias_type("AF", f32(), 601);
        let alias_bool = alias_type("AB", boolean(), 602);
        let alias_enum = alias_type("AE", default_enum(), 603);
        let alias_arr = alias_type("AA", anon_array(Some(3), i32()), 604);

        // is_int / is_float / is_numeric / is_primitive through aliases
        assert!(alias_i32.is_int());
        assert!(alias_f32.is_float());
        assert!(alias_i32.is_numeric());
        assert!(alias_i32.is_primitive());
        assert!(alias_bool.is_primitive());
        assert!(!alias_arr.is_primitive());

        // is_convertible_to_numeric: enums (and aliases of enums) are convertible
        assert!(alias_enum.is_convertible_to_numeric());
        assert!(default_enum().is_convertible_to_numeric());
        assert!(!string(None).is_convertible_to_numeric());

        // is_promotable_to_array: numeric/bool/string/enum yes; abs type no
        assert!(i32().is_promotable_to_array());
        assert!(boolean().is_promotable_to_array());
        assert!(string(None).is_promotable_to_array());
        assert!(default_enum().is_promotable_to_array());
        assert!(!default_abs_type().is_promotable_to_array());

        // array_size through alias / array / anon-array; None otherwise
        assert_eq!(alias_arr.array_size(), Some(3));
        assert_eq!(anon_array(Some(5), i32()).array_size(), Some(5));
        assert_eq!(
            array("A", anon_array(Some(2), i32()), 605).array_size(),
            Some(2)
        );
        assert_eq!(i32().array_size(), None);

        // has_numeric_members: arrays/structs of numerics yes; of strings no
        assert!(anon_array(Some(2), i32()).has_numeric_members());
        assert!(anon_struct(&[("x", i32()), ("y", f32())]).has_numeric_members());
        assert!(!anon_struct(&[("x", string(None))]).has_numeric_members());
        assert!(alias_i32.has_numeric_members());
    });
}

#[test]
fn type_default_values() {
    with_test_ctx(|| {
        // Primitive / scalar defaults.
        assert!(matches!(
            i32().default_value(),
            Some(Value::PrimitiveInteger(_))
        ));
        assert!(matches!(f64().default_value(), Some(Value::Float(_))));
        assert!(matches!(boolean().default_value(), Some(Value::Boolean(_))));
        assert!(matches!(
            string(None).default_value(),
            Some(Value::String(_))
        ));
        assert!(matches!(integer().default_value(), Some(Value::Integer(_))));

        // Alias delegates to its underlying type's default.
        assert!(matches!(
            alias_type("AI", i32(), 610).default_value(),
            Some(Value::PrimitiveInteger(_))
        ));

        // Sized anon array of a scalar has a default (repeated element).
        match anon_array(Some(3), u32()).default_value() {
            Some(Value::AnonArray(a)) => assert_eq!(a.elements.len(), 3),
            other => panic!("expected AnonArray default, got {other:?}"),
        }
        // Unsized anon array has no default.
        assert!(anon_array(None, u32()).default_value().is_none());

        // Anon struct default: each member defaulted.
        match anon_struct(&[("a", u32()), ("b", boolean())]).default_value() {
            Some(Value::AnonStruct(s)) => assert_eq!(s.members.len(), 2),
            other => panic!("expected AnonStruct default, got {other:?}"),
        }

        // Abstract type has no default value.
        assert!(default_abs_type().default_value().is_none());
    });
}
