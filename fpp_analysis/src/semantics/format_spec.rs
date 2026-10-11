#![cfg(test)]

use super::format::{Field, ParsedFormat, Parser};
use super::spec::*;
use crate::Analysis;
use crate::errors::SemanticError;
use crate::semantics::{
    Format, FormatReplacementKind, IntegerFormatKind, RationalFormatKind, Type,
};
use fpp_ast::LitString;
use fpp_core::{BytePos, Node, SourceFile, Span};
use std::sync::Arc;

/// A format string literal `"s"` in a source file of its own, so that its
/// fields have locations in the source
fn literal(s: &str) -> LitString {
    let src = SourceFile::new("format", format!("\"{s}\""));
    let span = Span::new(src, 0, s.len() as BytePos + 2, None);
    LitString {
        data: s.to_string(),
        inner_span: Span::new(src, 1, s.len() as BytePos, None),
        node_id: Node::new(span),
    }
}

/// Parse a string as a format
fn parse(s: &str) -> Result<ParsedFormat, SemanticError> {
    Parser::parse_node(&literal(s))
}

/// A parsed format as Scala's `Format(prefix, fields)`, without locations
fn format_of(f: &ParsedFormat) -> (String, Vec<(Field, String)>) {
    (
        f.prefix.clone(),
        f.fields
            .iter()
            .map(|(field, _, s)| (field.clone(), s.clone()))
            .collect(),
    )
}

fn format(prefix: &str, fields: Vec<(Field, &str)>) -> (String, Vec<(Field, String)>) {
    (
        prefix.to_string(),
        fields
            .into_iter()
            .map(|(field, s)| (field, s.to_string()))
            .collect(),
    )
}

fn integer_field(t: IntegerFormatKind) -> Field {
    Field::Integer(t)
}

fn rational_field(precision: Option<&str>, t: RationalFormatKind) -> Field {
    Field::Rational {
        precision: precision.map(str::to_string),
        kind: t,
    }
}

/// The message of an invalid format string
fn message(err: SemanticError) -> String {
    match err {
        SemanticError::InvalidFormatString { msg, .. } => msg,
        other => panic!("expected an invalid format string, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// "format" should parse <input> as <format>
// ---------------------------------------------------------------------------

#[test]
fn format_parses() {
    use IntegerFormatKind::*;
    use RationalFormatKind::*;
    with_test_ctx(|| {
        let ok = vec![
            ("abcd", format("abcd", vec![])),
            ("ab{{cd", format("ab{cd", vec![])),
            ("ab}}cd", format("ab}cd", vec![])),
            ("ab{}cd", format("ab", vec![(Field::Default, "cd")])),
            (
                "ab{c}cd",
                format("ab", vec![(integer_field(Character), "cd")]),
            ),
            (
                "ab{d}cd",
                format("ab", vec![(integer_field(Decimal), "cd")]),
            ),
            (
                "ab{x}cd",
                format("ab", vec![(integer_field(Hexadecimal), "cd")]),
            ),
            (
                "ab{e}cd",
                format("ab", vec![(rational_field(None, Exponent), "cd")]),
            ),
            (
                "ab{f}cd",
                format("ab", vec![(rational_field(None, Fixed), "cd")]),
            ),
            (
                "ab{g}cd",
                format("ab", vec![(rational_field(None, General), "cd")]),
            ),
            (
                "ab{.3e}cd",
                format("ab", vec![(rational_field(Some("3"), Exponent), "cd")]),
            ),
            (
                "ab{.3f}cd",
                format("ab", vec![(rational_field(Some("3"), Fixed), "cd")]),
            ),
            (
                "ab{.3g}cd",
                format("ab", vec![(rational_field(Some("3"), General), "cd")]),
            ),
        ];
        for (input, expected) in ok {
            match parse(input) {
                Ok(f) => assert_eq!(format_of(&f), expected, "parse {input}"),
                Err(err) => panic!("parse {input}: {err:?}"),
            }
        }
    });
}

// ---------------------------------------------------------------------------
// "format" should not parse <input>
// ---------------------------------------------------------------------------

#[test]
fn format_does_not_parse() {
    with_test_ctx(|| {
        for input in ["{", "}", "ab{1234xyz}cd", "ab{.3b}cd"] {
            assert!(parse(input).is_err(), "not parse {input}");
        }
    });
}

// ---------------------------------------------------------------------------
// Rust-only
// ---------------------------------------------------------------------------

/// The parser reports the first failure with Scala's message
#[test]
fn parse_errors_have_the_scala_messages() {
    with_test_ctx(|| {
        let cases = [
            ("{", "invalid replacement field"),
            ("}", "illegal character"),
            ("a}b", "illegal character"),
            ("{}}", "illegal character"),
            ("x}}}", "illegal character"),
            ("ab{1234xyz}cd", "invalid replacement field"),
            ("{ }", "invalid replacement field"),
            ("{{{", "invalid replacement field"),
            ("trailing {", "invalid replacement field"),
            ("ab{.3b}cd", "e, f, or g expected"),
            ("{.3", "e, f, or g expected"),
            ("ab{c", "'}' expected but end of source found"),
            ("{d", "'}' expected but end of source found"),
            ("{dx}", "'}' expected but 'x' found"),
            (
                "v={.}",
                "string matching regex '[0-9]+' expected but '}' found",
            ),
            (
                "{.",
                "string matching regex '[0-9]+' expected but end of source found",
            ),
            (
                "{e}{.}",
                "string matching regex '[0-9]+' expected but '}' found",
            ),
        ];
        for (input, expected) in cases {
            match parse(input) {
                Ok(f) => panic!("{input}: expected an error, got {f:?}"),
                Err(err) => assert_eq!(message(err), expected, "{input}"),
            }
        }
    });
}

/// A parse error at a multi-byte char is located at that char
#[test]
fn parse_errors_at_multi_byte_chars() {
    with_test_ctx(|| {
        for (input, expected, offset) in [
            ("{é}", "invalid replacement field", 1),
            ("{dé}", "'}' expected but 'é' found", 2),
            ("{.3é}", "e, f, or g expected", 3),
            ("ä{é}", "invalid replacement field", 3),
        ] {
            match parse(input) {
                Ok(f) => panic!("{input}: expected an error, got {f:?}"),
                Err(SemanticError::InvalidFormatString { loc, msg, .. }) => {
                    assert_eq!(msg, expected, "{input}");
                    // The location is the char, after the opening quote
                    assert_eq!(loc.start().pos(), offset + 1, "{input}");
                    assert_eq!(loc.len(), 'é'.len_utf8(), "{input}");
                }
                Err(err) => panic!("{input}: {err:?}"),
            }
        }
    });
}

/// A NUL char is a char of the string like any other
#[test]
fn nul_does_not_end_the_string() {
    with_test_ctx(|| {
        match parse("{}\0{q}") {
            Ok(f) => panic!("expected an error, got {f:?}"),
            Err(err) => assert_eq!(message(err), "invalid replacement field"),
        }
        let f = parse("a\0{}").expect("parses");
        assert_eq!(format_of(&f), format("a\0", vec![(Field::Default, "")]));
    });
}

#[test]
fn parses_octal_specifier() {
    with_test_ctx(|| {
        let f = parse("ab{o}cd").expect("parses");
        assert_eq!(
            format_of(&f),
            format("ab", vec![(integer_field(IntegerFormatKind::Octal), "cd")])
        );
    });
}

#[test]
fn utf8_literals_parse() {
    with_test_ctx(|| {
        let cases = [
            ("café {}", format("café ", vec![(Field::Default, "")])),
            ("temp {} °C", format("temp ", vec![(Field::Default, " °C")])),
            ("{} µ", format("", vec![(Field::Default, " µ")])),
            ("→λ→", format("→λ→", vec![])),
        ];
        for (input, expected) in cases {
            let f = parse(input).expect("parses");
            assert_eq!(format_of(&f), expected, "{input}");
        }
    });
}

/// A precision is kept as written, with no bound, until it is checked
#[test]
fn precision_is_unbounded() {
    with_test_ctx(|| {
        let large = "99999999999999999999999999999999999999999";
        for (input, precision) in [
            ("{.0100f}".to_string(), "100"),
            ("{.000f}".to_string(), "0"),
            (format!("{{.{large}f}}"), large),
        ] {
            let f = parse(&input).expect("parses");
            assert_eq!(
                format_of(&f),
                format(
                    "",
                    vec![(
                        rational_field(Some(precision), RationalFormatKind::Fixed),
                        ""
                    )]
                ),
                "{input}"
            );
        }
    });
}

/// `compute_format` checks the number of fields, and then each field against
/// its type: the integer check, then the precision, then the rational check
#[test]
fn compute_format_checks() {
    with_test_ctx(|| {
        let compute = |s: &str, ts: &[Arc<Type>]| Analysis::compute_format(&literal(s), ts);
        let error = |s: &str, ts: &[Arc<Type>]| match compute(s, ts) {
            Ok(f) => panic!("{s}: expected an error, got {f:?}"),
            Err(err) => message(err),
        };
        assert_eq!(error("abc", &[u32()]), "missing replacement field");
        assert_eq!(error("", &[u32()]), "missing replacement field");
        assert_eq!(error("{}{}", &[u32()]), "too many replacement fields");
        assert_eq!(
            error("{.200f}", &[u32(), u32()]),
            "missing replacement field"
        );
        assert_eq!(error("{d}", &[f32()]), "F32 is not an integer type");
        assert_eq!(error("{f}", &[u32()]), "U32 is not a floating-point type");
        assert_eq!(error("{x}", &[boolean()]), "bool is not an integer type");
        assert_eq!(
            error("{.200f}", &[u32()]),
            "precision value 200 is out of range"
        );
        assert_eq!(
            error("{.99999999999f}", &[f32()]),
            "precision value 99999999999 is out of range"
        );
        // The first field that does not check is reported
        assert_eq!(
            error("{d} {x}", &[f32(), f64()]),
            "F32 is not an integer type"
        );
        assert_eq!(
            error("{d} {.200f}", &[u32(), u32()]),
            "precision value 200 is out of range"
        );

        assert!(compute("{.0100f}", &[f32()]).is_ok());
        assert!(compute("{.100e}", &[f64()]).is_ok());
        assert!(compute("{} {d} {f}", &[boolean(), u8(), f32()]).is_ok());
        assert!(compute("{d}", &[integer()]).is_ok());
    });
}

#[test]
fn format_accessors() {
    with_test_ctx(|| {
        let format_of = |s: &str| -> Format { parse(s).expect("parses").into() };

        // No fields: len 0, empty, get(0) is None.
        let f0 = format_of("no fields here");
        assert_eq!(f0.len(), 0);
        assert!(f0.is_empty());
        assert!(f0.get(0).is_none());
        assert_eq!(f0.iter().count(), 0);

        // Two fields: len 2, not empty, get(0)/get(1) present, get(2) None.
        let f2 = format_of("{d} and {.3f}");
        assert_eq!(f2.len(), 2);
        assert!(!f2.is_empty());
        assert_eq!(
            f2.get(0).map(|f| f.kind.clone()),
            Some(FormatReplacementKind::Integer(IntegerFormatKind::Decimal))
        );
        assert_eq!(
            f2.get(1).map(|f| f.kind.clone()),
            Some(FormatReplacementKind::Rational {
                precision: Some(3),
                kind: RationalFormatKind::Fixed
            })
        );
        assert!(f2.get(2).is_none());
        assert_eq!(f2.iter().count(), 2);
    });
}
