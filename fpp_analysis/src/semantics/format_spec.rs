#![cfg(test)]

use crate::semantics::{
    Format, FormatPart, FormatReplacementKind, IntegerFormatKind, RationalFormatKind,
};
use fpp_ast::LitString;
use fpp_core::{CompilerContext, DiagnosticData, DiagnosticEmitter, Level, Node, SourceFile, Span};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Comparable, span-free shape for Format's parsed representation.
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Clone)]
enum Shape {
    Literal(String),
    Default,
    Integer(IntKind),
    Rational(Option<i32>, RatKind),
}

#[derive(Debug, PartialEq, Clone)]
enum IntKind {
    Character,
    Decimal,
    Hexadecimal,
    Octal,
}

#[derive(Debug, PartialEq, Clone)]
enum RatKind {
    Exponent,
    Fixed,
    General,
}

fn shape(p: &FormatPart) -> Shape {
    match p {
        FormatPart::Literal(s) => Shape::Literal(s.clone()),
        FormatPart::FormatReplacement(f) => match &f.kind {
            FormatReplacementKind::Default => Shape::Default,
            FormatReplacementKind::Integer(k) => Shape::Integer(match k {
                IntegerFormatKind::Character => IntKind::Character,
                IntegerFormatKind::Decimal => IntKind::Decimal,
                IntegerFormatKind::Hexadecimal => IntKind::Hexadecimal,
                IntegerFormatKind::Octal => IntKind::Octal,
            }),
            FormatReplacementKind::Rational { precision, kind } => Shape::Rational(
                *precision,
                match kind {
                    RationalFormatKind::Exponent => RatKind::Exponent,
                    RationalFormatKind::Fixed => RatKind::Fixed,
                    RationalFormatKind::General => RatKind::General,
                },
            ),
        },
    }
}

// ---------------------------------------------------------------------------
// Error-counting emitter: follows the same `WriteEmitter`-buffer pattern used
// in `types.rs`, but keeps a precise count of error-level diagnostics so we
// don't have to string-match rendered output.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct CountingEmitter {
    errors: Arc<std::sync::atomic::AtomicUsize>,
}

impl DiagnosticEmitter for CountingEmitter {
    fn emit(&mut self, diagnostic: DiagnosticData) {
        if diagnostic.level == Level::Error {
            self.errors
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

/// Result of parsing a format string with **no** expected types.
struct ParseResult {
    shapes: Vec<Shape>,
    /// Count of error-level diagnostics emitted while parsing.
    ///
    /// With an empty type list, the only non-parser error `Format::new` can
    /// raise is the length-mismatch (one field vs. zero types). We subtract
    /// those out here so `parser_errors` reflects *only* the format parser's
    /// own diagnostics.
    parser_errors: usize,
}

fn parse(input: &str) -> ParseResult {
    let errors = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut ctx = CompilerContext::new(CountingEmitter {
        errors: errors.clone(),
    });
    let shapes = fpp_core::run(&mut ctx, || {
        let src = SourceFile::new("format", input.to_string());
        // A non-zero length span is required by the underlying span machinery.
        let span = Span::new(src, 0, input.len().max(1) as u32, None);
        let lit = LitString {
            data: input.to_string(),
            inner_span: span,
            node_id: Node::new(span),
        };
        // Parse with no expected types: this exercises only the parser.
        let format = Format::new(&lit, vec![]);
        format.0.iter().map(shape).collect::<Vec<_>>()
    });
    drop(ctx);

    // Number of replacement fields actually produced by the parser. With an
    // empty type list, `Format::new` emits exactly one length-mismatch error
    // whenever there is at least one field, so subtract that one out to
    // recover the parser's own error count.
    let field_count = shapes
        .iter()
        .filter(|s| !matches!(s, Shape::Literal(_)))
        .count();
    let total = errors.load(std::sync::atomic::Ordering::SeqCst);
    let length_mismatch_errors = usize::from(field_count > 0);
    let parser_errors = total.saturating_sub(length_mismatch_errors);

    ParseResult {
        shapes,
        parser_errors,
    }
}

// ---------------------------------------------------------------------------
// "format" should parse <input> as <expected>
// ---------------------------------------------------------------------------

#[test]
fn parses_ok_cases() {
    // (input, expected span-free shape) pairs. The parsed representation
    // flattens to a `Vec` of interleaved literals and replacement fields.
    let ok: Vec<(&str, Vec<Shape>)> = vec![
        ("abcd", vec![Shape::Literal("abcd".into())]),
        ("ab{{cd", vec![Shape::Literal("ab{cd".into())]),
        ("ab}}cd", vec![Shape::Literal("ab}cd".into())]),
        (
            "ab{}cd",
            vec![
                Shape::Literal("ab".into()),
                Shape::Default,
                Shape::Literal("cd".into()),
            ],
        ),
        (
            "ab{c}cd",
            vec![
                Shape::Literal("ab".into()),
                Shape::Integer(IntKind::Character),
                Shape::Literal("cd".into()),
            ],
        ),
        (
            "ab{d}cd",
            vec![
                Shape::Literal("ab".into()),
                Shape::Integer(IntKind::Decimal),
                Shape::Literal("cd".into()),
            ],
        ),
        (
            "ab{x}cd",
            vec![
                Shape::Literal("ab".into()),
                Shape::Integer(IntKind::Hexadecimal),
                Shape::Literal("cd".into()),
            ],
        ),
        (
            "ab{e}cd",
            vec![
                Shape::Literal("ab".into()),
                Shape::Rational(None, RatKind::Exponent),
                Shape::Literal("cd".into()),
            ],
        ),
        (
            "ab{f}cd",
            vec![
                Shape::Literal("ab".into()),
                Shape::Rational(None, RatKind::Fixed),
                Shape::Literal("cd".into()),
            ],
        ),
        (
            "ab{g}cd",
            vec![
                Shape::Literal("ab".into()),
                Shape::Rational(None, RatKind::General),
                Shape::Literal("cd".into()),
            ],
        ),
        (
            "ab{.3e}cd",
            vec![
                Shape::Literal("ab".into()),
                Shape::Rational(Some(3), RatKind::Exponent),
                Shape::Literal("cd".into()),
            ],
        ),
        (
            "ab{.3f}cd",
            vec![
                Shape::Literal("ab".into()),
                Shape::Rational(Some(3), RatKind::Fixed),
                Shape::Literal("cd".into()),
            ],
        ),
        (
            "ab{.3g}cd",
            vec![
                Shape::Literal("ab".into()),
                Shape::Rational(Some(3), RatKind::General),
                Shape::Literal("cd".into()),
            ],
        ),
    ];

    for (input, expected) in ok {
        let result = parse(input);
        assert_eq!(
            result.parser_errors, 0,
            "expected `{input}` to parse without parser errors, got {} error(s)",
            result.parser_errors
        );
        assert_eq!(
            result.shapes, expected,
            "parsed shape mismatch for input `{input}`"
        );
    }
}

// ---------------------------------------------------------------------------
// "format" should not parse <input>
// ---------------------------------------------------------------------------

#[test]
fn rejects_error_cases() {
    // These inputs fail to parse; the failure surfaces as one or more
    // error-level diagnostics emitted by the format parser.
    let error_inputs = ["{", "}", "ab{1234xyz}cd", "ab{.3b}cd"];

    for input in error_inputs {
        let result = parse(input);
        assert!(
            result.parser_errors > 0,
            "expected `{input}` to fail parsing (emit a parser error), but none were emitted; \
             parsed shape: {:?}",
            result.shapes
        );
    }
}

#[test]
fn utf8_literals_do_not_panic() {
    let cases = [
        (
            "café {}",
            vec![Shape::Literal("café ".into()), Shape::Default],
        ),
        (
            "temp {} °C",
            vec![
                Shape::Literal("temp ".into()),
                Shape::Default,
                Shape::Literal(" °C".into()),
            ],
        ),
        ("{} µ", vec![Shape::Default, Shape::Literal(" µ".into())]),
        // Multibyte chars only, no replacement field.
        ("→λ→", vec![Shape::Literal("→λ→".into())]),
    ];

    for (input, expected) in cases {
        let result = parse(input);
        assert_eq!(
            result.parser_errors, 0,
            "unexpected parser error for `{input}`"
        );
        assert_eq!(result.shapes, expected, "shape mismatch for `{input}`");
    }
}

// ---------------------------------------------------------------------------
// Additional coverage: the octal specifier, error-recovery cases, and the
// public `Format` accessor methods (`len`/`is_empty`/`get`/`iter`).
// ---------------------------------------------------------------------------

/// Builds a `Format` from `input` with no expected types, for exercising the
/// public accessors directly.
fn build_format(input: &str) -> (Format, usize) {
    let errors = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut ctx = CompilerContext::new(CountingEmitter {
        errors: errors.clone(),
    });
    let format = fpp_core::run(&mut ctx, || {
        let src = SourceFile::new("format", input.to_string());
        let span = Span::new(src, 0, input.len().max(1) as u32, None);
        let lit = LitString {
            data: input.to_string(),
            inner_span: span,
            node_id: Node::new(span),
        };
        Format::new(&lit, vec![])
    });
    drop(ctx);
    (format, errors.load(std::sync::atomic::Ordering::SeqCst))
}

#[test]
fn parses_octal_specifier() {
    let result = parse("ab{o}cd");
    assert_eq!(result.parser_errors, 0);
    assert_eq!(
        result.shapes,
        vec![
            Shape::Literal("ab".into()),
            Shape::Integer(IntKind::Octal),
            Shape::Literal("cd".into()),
        ]
    );
}

#[test]
fn format_accessors() {
    // No fields: len 0, empty, get(0) is None.
    let (f0, _) = build_format("no fields here");
    assert_eq!(f0.len(), 0);
    assert!(f0.is_empty());
    assert!(f0.get(0).is_none());
    assert_eq!(f0.iter().count(), 0);

    // Two fields: len 2, not empty, get(0)/get(1) present, get(2) None.
    let (f2, _) = build_format("{d} and {x}");
    assert_eq!(f2.len(), 2);
    assert!(!f2.is_empty());
    assert!(f2.get(0).is_some());
    assert!(f2.get(1).is_some());
    assert!(f2.get(2).is_none());
    assert_eq!(f2.iter().count(), 2);
}

#[test]
fn rejects_more_error_recovery_cases() {
    // Unclosed field (`{c` with no `}`): parser emits an error.
    assert!(parse("ab{c").parser_errors > 0);
    // Empty precision (`{.}`): parser emits an error.
    assert!(parse("v={.}").parser_errors > 0);
    // Bare `{` at end of string.
    assert!(parse("trailing {").parser_errors > 0);
}
