use crate::errors::SemanticError;
use crate::semantics::Type;
use fpp_ast::LitString;
use fpp_core::BytePos;
use std::str::{Chars, FromStr};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum IntegerFormatKind {
    Character,
    Decimal,
    Hexadecimal,
    Octal,
}

#[derive(Debug, Clone)]
pub enum RationalFormatKind {
    Exponent,
    Fixed,
    General,
}

#[derive(Debug, Clone)]
pub enum FormatReplacementKind {
    Default,
    Integer(IntegerFormatKind),
    Rational {
        precision: Option<i32>,
        kind: RationalFormatKind,
    },
}

#[derive(Debug, Clone)]
pub struct FormatReplacementField {
    pub span: fpp_core::Span,
    pub kind: FormatReplacementKind,
}

#[derive(Debug, Clone)]
pub enum FormatPart {
    Literal(String),
    FormatReplacement(FormatReplacementField),
}

/// An FPP presentation format
#[derive(Debug, Clone)]
pub struct Format(pub Vec<FormatPart>);

impl Format {
    pub fn new(node: &LitString, ts: Vec<(Arc<Type>, fpp_core::Span)>) -> Format {
        let format = Format(FormatParser::new(node.inner_span, &node.data).collect());

        // Validate the fields in the format string against the types expected to be formated
        if format.len() != ts.len() {
            SemanticError::FormatStringMismatchLength {
                format_locs: format.iter().map(|f| f.span).collect(),
                type_locs: ts.iter().map(|t| t.1).collect(),
            }
            .emit();
        } else {
            // Validate all the arguments against their format fields
            std::iter::zip(format.iter(), ts.iter()).for_each(|(format_spec, (ty, ty_span))| {
                match &format_spec.kind {
                    // This can format anything
                    FormatReplacementKind::Default => {}
                    FormatReplacementKind::Integer(kind) => {
                        match Type::underlying_type(ty).as_ref() {
                            Type::PrimitiveInt(_) => {}
                            Type::Integer => {}
                            _ => SemanticError::FormatStringInvalidReplacement {
                                format_loc: format_spec.span,
                                type_loc: *ty_span,
                                msg: format!(
                                    "{:?} format replacement cannot be used for type `{}`",
                                    kind, ty
                                ),
                            }
                            .emit(),
                        }
                    }
                    FormatReplacementKind::Rational { kind, precision } => {
                        match *precision {
                            None => {}
                            // The maximum allowed precision is 100
                            Some(precision) if precision > 100 => {
                                SemanticError::FormatStringInvalidPrecision {
                                    loc: format_spec.span,
                                    value: precision,
                                    max: 100,
                                }
                                .emit();
                            }
                            Some(_) => {}
                        }

                        match Type::underlying_type(ty).as_ref() {
                            Type::Float(_) => {}
                            _ => SemanticError::FormatStringInvalidReplacement {
                                format_loc: format_spec.span,
                                type_loc: *ty_span,
                                msg: format!(
                                    "{:?} format replacement cannot be used for type `{}`",
                                    kind, ty
                                ),
                            }
                            .emit(),
                        }
                    }
                }
            });
        }

        format
    }

    /// Number of fields in the format string
    pub fn len(&self) -> usize {
        self.0
            .iter()
            .filter(|f| matches!(f, FormatPart::FormatReplacement(_)))
            .count()
    }

    /// Whether the format string has no replacement fields
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, n: usize) -> Option<&FormatReplacementField> {
        self.0
            .iter()
            .filter_map(|f| match f {
                FormatPart::FormatReplacement(f) => Some(f),
                _ => None,
            })
            .nth(n)
    }

    pub fn iter(&self) -> impl Iterator<Item = &FormatReplacementField> {
        self.0.iter().filter_map(|f| match f {
            FormatPart::FormatReplacement(f) => Some(f),
            _ => None,
        })
    }
}

/// Parses a format string into a format
struct FormatParser<'a> {
    parent_span: fpp_core::Span,
    pos: BytePos,
    len_remaining: usize,

    literal: String,

    /// Iterator over chars. Slightly faster than a &str.
    chars: Chars<'a>,
}

const EOF_CHAR: char = '\0';

enum LexerResult {
    Eof,
    KeepGoing,
    Literal(String),
    FormatReplacement(FormatReplacementKind),
}

impl<'a> Iterator for FormatParser<'a> {
    type Item = FormatPart;

    fn next(&mut self) -> Option<FormatPart> {
        self.pos += self.pos_within_token();
        self.reset_token();

        let start = self.pos;
        loop {
            match self.next_kind() {
                LexerResult::Eof => return None,
                LexerResult::KeepGoing => {}
                LexerResult::Literal(s) => return Some(FormatPart::Literal(s)),
                LexerResult::FormatReplacement(kind) => {
                    let length = self.pos_within_token();
                    return Some(FormatPart::FormatReplacement(FormatReplacementField {
                        span: fpp_core::Span::new(
                            self.parent_span.file(),
                            self.parent_span.start().pos() + start,
                            length,
                            self.parent_span.including_span(),
                        ),
                        kind,
                    }));
                }
            }
        }
    }
}

impl<'a> FormatParser<'a> {
    fn new(span: fpp_core::Span, content: &'a str) -> FormatParser<'a> {
        let chars = content.chars();
        FormatParser {
            parent_span: span,
            pos: 0,
            len_remaining: content.len(),
            literal: String::new(),
            chars,
        }
    }

    fn next_kind(&mut self) -> LexerResult {
        match self.first() {
            EOF_CHAR => {
                if self.literal.is_empty() {
                    LexerResult::Eof
                } else {
                    LexerResult::Literal(self.literal.clone())
                }
            }
            '{' => {
                // Check if this '{' is escaped
                if self.second() == '{' {
                    self.bump();
                    self.bump();
                    self.literal.push('{');
                    LexerResult::KeepGoing
                } else if self.literal.is_empty() && self.second() == '}' {
                    self.bump();
                    self.bump();
                    LexerResult::FormatReplacement(FormatReplacementKind::Default)
                } else if self.literal.is_empty() {
                    self.bump();
                    match self.field() {
                        None => {
                            // The field failed to be parsed
                            LexerResult::KeepGoing
                        }
                        Some(f) => {
                            // Close out the '}'
                            match self.bump() {
                                None => {
                                    self.error("unclosed format replacement").emit();
                                }
                                Some('}') => {}
                                Some(_) => {
                                    self.error("expected '}' to close format replacement")
                                        .emit();
                                }
                            }

                            LexerResult::FormatReplacement(f)
                        }
                    }
                } else {
                    // Flush out the literal on the left side of the token
                    LexerResult::Literal(self.literal.clone())
                }
            }
            '}' => {
                if self.second() == '}' {
                    self.bump();
                    self.bump();
                    self.literal.push('}');
                } else {
                    self.bump();
                    self.error("unmatched `}` in format string")
                        .note("consider escaping curly brace with `}}`")
                        .emit()
                }

                LexerResult::KeepGoing
            }
            c => {
                self.bump();
                self.literal.push(c);
                LexerResult::KeepGoing
            }
        }
    }

    fn field(&mut self) -> Option<FormatReplacementKind> {
        let c = match self.bump() {
            None => {
                self.error("unmatched `{` in format string")
                    .note("consider escaping curly brace with `{{`")
                    .emit();
                return None;
            }
            Some(c) => c,
        };

        match c {
            '.' => {
                // Precision prefix
                let mut precision = vec![];
                while let d @ '0'..='9' = self.first() {
                    self.bump();
                    precision.push(d as u8)
                }

                if precision.is_empty() {
                    self.error("expected precision field in format").emit();
                    None
                } else {
                    let p = match i32::from_str(&String::from_utf8(precision).unwrap()) {
                        Ok(i) => i,
                        Err(err) => {
                            self.error("invalid floating precision")
                                .annotation(err.to_string())
                                .emit();
                            return None;
                        }
                    };

                    let n = match self.bump() {
                        None => {
                            self.error("unexpected end of string")
                                .note("consider escaping curly brace with `{{`")
                                .emit();
                            return None;
                        }
                        Some(c) => c,
                    };

                    match n {
                        'e' => Some(FormatReplacementKind::Rational {
                            precision: Some(p),
                            kind: RationalFormatKind::Exponent,
                        }),
                        'f' => Some(FormatReplacementKind::Rational {
                            precision: Some(p),
                            kind: RationalFormatKind::Fixed,
                        }),
                        'g' => Some(FormatReplacementKind::Rational {
                            precision: Some(p),
                            kind: RationalFormatKind::General,
                        }),
                        _ => {
                            self.error(
                                "expected rational format replacement field ('e', 'f', 'g')",
                            )
                            .note("precision modifiers only support rational replacement fields")
                            .emit();
                            None
                        }
                    }
                }
            }
            'c' => Some(FormatReplacementKind::Integer(IntegerFormatKind::Character)),
            'd' => Some(FormatReplacementKind::Integer(IntegerFormatKind::Decimal)),
            'x' => Some(FormatReplacementKind::Integer(
                IntegerFormatKind::Hexadecimal,
            )),
            'o' => Some(FormatReplacementKind::Integer(IntegerFormatKind::Octal)),
            'e' => Some(FormatReplacementKind::Rational {
                precision: None,
                kind: RationalFormatKind::Exponent,
            }),
            'f' => Some(FormatReplacementKind::Rational {
                precision: None,
                kind: RationalFormatKind::Fixed,
            }),
            'g' => Some(FormatReplacementKind::Rational {
                precision: None,
                kind: RationalFormatKind::General,
            }),
            _ => {
                self.error("invalid format replacement field").emit();
                None
            }
        }
    }

    fn error(&self, msg: &str) -> fpp_core::Diagnostic {
        fpp_core::Diagnostic::new(
            fpp_core::Span::new(
                self.parent_span.file(),
                self.parent_span.start().pos() + self.pos + self.pos_within_token() - 1,
                1,
                self.parent_span.including_span(),
            ),
            fpp_core::Level::Error,
            msg,
        )
    }

    /// Peeks the next symbol from the input stream without consuming it.
    /// If requested position doesn't exist, `EOF_CHAR` is returned.
    /// However, getting `EOF_CHAR` doesn't always mean actual end of file,
    /// it should be checked with `is_eof` method.
    fn first(&self) -> char {
        // `.next()` optimizes better than `.nth(0)`
        self.chars.clone().next().unwrap_or(EOF_CHAR)
    }

    /// Peeks the second symbol from the input stream without consuming it.
    fn second(&self) -> char {
        // `.next()` optimizes better than `.nth(1)`
        let mut iter = self.chars.clone();
        iter.next();
        iter.next().unwrap_or(EOF_CHAR)
    }

    /// Moves to the next character.
    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;

        Some(c)
    }

    /// Resets the number of bytes consumed to 0.
    fn reset_token(&mut self) {
        self.len_remaining = self.chars.as_str().len();
        self.literal.clear();
    }

    /// Returns amount of already consumed symbols.
    fn pos_within_token(&self) -> BytePos {
        (self.len_remaining - self.chars.as_str().len()) as BytePos
    }
}
