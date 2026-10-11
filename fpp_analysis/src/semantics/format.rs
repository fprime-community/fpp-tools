use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::Type;
use fpp_ast::LitString;
use fpp_core::Spanned;
use std::sync::Arc;

/// The type of an integer replacement field
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegerFormatKind {
    Character,
    Decimal,
    Hexadecimal,
    Octal,
}

/// The type of a rational replacement field
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RationalFormatKind {
    Exponent,
    Fixed,
    General,
}

/// The kind of a replacement field
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatReplacementKind {
    Default,
    Integer(IntegerFormatKind),
    Rational {
        precision: Option<i32>,
        kind: RationalFormatKind,
    },
}

/// A replacement field of a format, with its location
#[derive(Debug, Clone)]
pub struct FormatReplacementField {
    /// The location of the field: the field itself when its position in the
    /// source is known, otherwise the format string
    pub span: fpp_core::Span,
    pub kind: FormatReplacementKind,
}

/// A part of a format: a literal string or a replacement field
#[derive(Debug, Clone)]
pub enum FormatPart {
    Literal(String),
    FormatReplacement(FormatReplacementField),
}

/// An FPP presentation format
#[derive(Debug, Clone)]
pub struct Format(pub Vec<FormatPart>);

impl Format {
    /// Computes the format of a format string for the types of the values it
    /// formats, reporting an invalid format string. A format string that is
    /// not valid for the types has the empty format. The type-name spans are
    /// not used.
    ///
    /// See [`Analysis::compute_format`].
    pub fn new(node: &LitString, ts: Vec<(Arc<Type>, fpp_core::Span)>) -> Format {
        let ts: Vec<Arc<Type>> = ts.into_iter().map(|(t, _)| t).collect();
        match Analysis::compute_format(node, &ts) {
            Ok(format) => format,
            Err(err) => {
                err.emit();
                Format(vec![])
            }
        }
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

    /// The replacement field at index `n`, if any
    pub fn get(&self, n: usize) -> Option<&FormatReplacementField> {
        self.iter().nth(n)
    }

    /// The replacement fields, in order
    pub fn iter(&self) -> impl Iterator<Item = &FormatReplacementField> {
        self.0.iter().filter_map(|f| match f {
            FormatPart::FormatReplacement(f) => Some(f),
            _ => None,
        })
    }
}

/// A replacement field, as the format parser reads it
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Field {
    Default,
    Integer(IntegerFormatKind),
    /// The precision is the value as written, in decimal digits with no
    /// leading zeros: it has no bound until the format is checked against its
    /// types.
    Rational {
        precision: Option<String>,
        kind: RationalFormatKind,
    },
}

impl Field {
    /// The maximum allowed precision
    pub(crate) const MAX_PRECISION: u32 = 100;

    pub(crate) fn is_integer(&self) -> bool {
        matches!(self, Field::Integer(_))
    }

    pub(crate) fn is_rational(&self) -> bool {
        matches!(self, Field::Rational { .. })
    }

    /// Whether a precision, given as decimal digits with no leading zeros, is
    /// larger than [`Field::MAX_PRECISION`]
    fn exceeds_max_precision(precision: &str) -> bool {
        precision
            .parse::<u32>()
            .map_or(true, |p| p > Field::MAX_PRECISION)
    }

    fn kind(&self) -> FormatReplacementKind {
        match self {
            Field::Default => FormatReplacementKind::Default,
            Field::Integer(kind) => FormatReplacementKind::Integer(kind.clone()),
            Field::Rational { precision, kind } => FormatReplacementKind::Rational {
                precision: precision
                    .as_ref()
                    .map(|p| p.parse::<i32>().unwrap_or(i32::MAX)),
                kind: kind.clone(),
            },
        }
    }
}

/// A format as the format parser reads it
#[derive(Debug, Clone)]
pub(crate) struct ParsedFormat {
    /// The first part of the format, before any fields
    pub(crate) prefix: String,
    /// The list of pairs of fields followed by suffix strings, with the
    /// location of each field
    pub(crate) fields: Vec<(Field, fpp_core::Span, String)>,
}

impl From<ParsedFormat> for Format {
    fn from(format: ParsedFormat) -> Format {
        let mut parts = vec![];
        if !format.prefix.is_empty() {
            parts.push(FormatPart::Literal(format.prefix));
        }
        for (field, span, suffix) in format.fields {
            parts.push(FormatPart::FormatReplacement(FormatReplacementField {
                span,
                kind: field.kind(),
            }));
            if !suffix.is_empty() {
                parts.push(FormatPart::Literal(suffix));
            }
        }
        Format(parts)
    }
}

impl Analysis {
    /// Compute a format from a format string and a list of types
    pub fn compute_format(node: &LitString, ts: &[Arc<Type>]) -> SemanticResult<Format> {
        let loc = node.span();
        let format = Parser::parse_node(node)?;
        // Check the size
        if format.fields.len() < ts.len() {
            return Err(SemanticError::InvalidFormatString {
                loc,
                msg: "missing replacement field".to_string(),
                context: None,
            });
        }
        if format.fields.len() > ts.len() {
            return Err(SemanticError::InvalidFormatString {
                loc: format.fields[ts.len()].1,
                msg: "too many replacement fields".to_string(),
                context: None,
            });
        }
        // Check each numeric field against its type
        for (t, (field, field_loc, _)) in ts.iter().zip(format.fields.iter()) {
            let error = |msg: String| SemanticError::InvalidFormatString {
                loc: *field_loc,
                msg,
                context: None,
            };
            let u = Type::underlying_type(t);
            if field.is_integer() && !u.is_int() {
                return Err(error(format!("{t} is not an integer type")));
            }
            if let Field::Rational {
                precision: Some(precision),
                ..
            } = field
                && Field::exceeds_max_precision(precision)
            {
                return Err(error(format!(
                    "precision value {precision} is out of range"
                )));
            }
            if field.is_rational() && !u.is_float() {
                return Err(error(format!("{t} is not a floating-point type")));
            }
        }
        Ok(format.into())
    }
}

/// A failure to parse a format string: the message and the position (a char
/// index into the string) at which the parser failed
struct ParseError {
    msg: String,
    pos: usize,
}

type ParseResult<T> = Result<T, ParseError>;

/// Parse a format string into a format
///
/// This is a recursive descent parser for the grammar
///
/// ```text
/// format    = string { field string }
/// string    = { any char except '{' and '}' | "{{" | "}}" }
/// field     = "{}" | "{" ( integer | rational ) "}"
/// integer   = "c" | "d" | "x" | "o"
/// rational  = [ precision ] ( "e" | "f" | "g" )
/// precision = "." digit { digit }
/// ```
///
/// The whole string must be a format. A failure after a field's opening `{`
/// is final; at the first failure the parser reports the error and stops.
pub(crate) struct Parser<'a> {
    /// The node of the format string
    node: &'a LitString,
    /// The chars of the format string, with their byte offsets
    chars: Vec<(usize, char)>,
    /// The index of the next char to read
    pos: usize,
    /// Whether the text of the literal is the format string verbatim, so that
    /// a byte offset into the string is the same offset into the source text
    /// that follows the opening quotes
    verbatim: bool,
}

impl<'a> Parser<'a> {
    fn new(node: &'a LitString) -> Parser<'a> {
        Parser {
            node,
            chars: node.data.char_indices().collect(),
            pos: 0,
            verbatim: node.inner_span.len() == node.data.len()
                // A single-line literal: the text between the quotes is all of
                // the literal except its two quotes
                && node.span().len() == node.inner_span.len() + 2,
        }
    }

    /// Parse the format string of a node
    pub(crate) fn parse_node(node: &LitString) -> SemanticResult<ParsedFormat> {
        let mut parser = Parser::new(node);
        match parser.format() {
            Ok(format) => Ok(format),
            Err(ParseError { msg, pos }) => Err(SemanticError::InvalidFormatString {
                loc: if parser.verbatim {
                    parser.loc(pos, pos + 1)
                } else {
                    node.span()
                },
                msg,
                context: (!parser.verbatim).then(|| parser.long_string(pos)),
            }),
        }
    }

    /// The whole input is a format
    fn format(&mut self) -> ParseResult<ParsedFormat> {
        let prefix = self.string();
        let mut fields = vec![];
        while let Some(field) = self.field()? {
            fields.push((field.0, field.1, self.string()));
        }
        if self.pos < self.chars.len() {
            // The string stops only at a '{' or '}', and a '{' starts a field,
            // so this is an unmatched '}'
            return Err(ParseError {
                msg: "illegal character".to_string(),
                pos: self.pos,
            });
        }
        Ok(ParsedFormat { prefix, fields })
    }

    /// A literal string, in which "{{" stands for '{' and "}}" for '}'
    fn string(&mut self) -> String {
        let mut out = String::new();
        loop {
            match (self.peek(0), self.peek(1)) {
                (Some('{'), Some('{')) => {
                    out.push('{');
                    self.pos += 2;
                }
                (Some('}'), Some('}')) => {
                    out.push('}');
                    self.pos += 2;
                }
                (Some('{' | '}'), _) | (None, _) => return out,
                (Some(c), _) => {
                    out.push(c);
                    self.pos += 1;
                }
            }
        }
    }

    /// A replacement field, or `None` if the input is not at a field
    fn field(&mut self) -> ParseResult<Option<(Field, fpp_core::Span)>> {
        let start = self.pos;
        if self.peek(0) != Some('{') {
            return Ok(None);
        }
        if self.peek(1) == Some('}') {
            self.pos += 2;
            return Ok(Some((Field::Default, self.loc(start, self.pos))));
        }
        self.pos += 1;
        let field = match self.peek(0) {
            Some('c') => Field::Integer(IntegerFormatKind::Character),
            Some('d') => Field::Integer(IntegerFormatKind::Decimal),
            Some('x') => Field::Integer(IntegerFormatKind::Hexadecimal),
            Some('o') => Field::Integer(IntegerFormatKind::Octal),
            Some('.') => {
                let precision = self.precision()?;
                Field::Rational {
                    precision: Some(precision),
                    kind: self.rational_type()?,
                }
            }
            Some('e' | 'f' | 'g') => Field::Rational {
                precision: None,
                kind: self.rational_type()?,
            },
            _ => {
                return Err(ParseError {
                    msg: "invalid replacement field".to_string(),
                    pos: self.pos,
                });
            }
        };
        if field.is_integer() {
            self.pos += 1;
        }
        match self.peek(0) {
            Some('}') => {
                self.pos += 1;
                Ok(Some((field, self.loc(start, self.pos))))
            }
            found => Err(ParseError {
                msg: format!("'}}' expected but {} found", Self::show(found)),
                pos: self.pos,
            }),
        }
    }

    /// A precision: '.' followed by decimal digits, as the digits with no
    /// leading zeros
    fn precision(&mut self) -> ParseResult<String> {
        self.pos += 1;
        let start = self.pos;
        while let Some('0'..='9') = self.peek(0) {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(ParseError {
                msg: format!(
                    "string matching regex '[0-9]+' expected but {} found",
                    Self::show(self.peek(0))
                ),
                pos: self.pos,
            });
        }
        let digits: String = self.chars[start..self.pos].iter().map(|(_, c)| c).collect();
        let digits = digits.trim_start_matches('0');
        Ok(if digits.is_empty() { "0" } else { digits }.to_string())
    }

    /// The type of a rational field
    fn rational_type(&mut self) -> ParseResult<RationalFormatKind> {
        let kind = match self.peek(0) {
            Some('e') => RationalFormatKind::Exponent,
            Some('f') => RationalFormatKind::Fixed,
            Some('g') => RationalFormatKind::General,
            _ => {
                return Err(ParseError {
                    msg: "e, f, or g expected".to_string(),
                    pos: self.pos,
                });
            }
        };
        self.pos += 1;
        Ok(kind)
    }

    /// The char at `offset` chars past the current position, if any
    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).map(|(_, c)| *c)
    }

    /// A char, or the end of the string, as the parser shows it in a message
    fn show(c: Option<char>) -> String {
        match c {
            Some(c) => format!("'{c}'"),
            None => "end of source".to_string(),
        }
    }

    /// The byte offset into the string of the char at index `pos`
    fn offset(&self, pos: usize) -> usize {
        self.chars
            .get(pos)
            .map_or(self.node.data.len(), |(offset, _)| *offset)
    }

    /// The location of the chars from index `start` up to `end`: those chars
    /// in the source when the literal is verbatim, otherwise the whole literal.
    /// At the end of the string, the location is the closing quote.
    fn loc(&self, start: usize, end: usize) -> fpp_core::Span {
        if !self.verbatim {
            return self.node.span();
        }
        let inner = self.node.inner_span;
        let start = self.offset(start);
        let len = (self.offset(end) - start).max(1);
        let span = fpp_core::Span::new(
            inner.file(),
            inner.start().pos() + start as fpp_core::BytePos,
            len as fpp_core::BytePos,
            inner.including_span(),
        );
        match inner.expanding_span() {
            Some(expansion) => span.expanded_at(expansion),
            None => span,
        }
    }

    /// The line of the string that holds the char at index `pos`, followed by
    /// a line with a caret under that char
    fn long_string(&self, pos: usize) -> String {
        let line_start = self.chars[..pos.min(self.chars.len())]
            .iter()
            .rposition(|(_, c)| *c == '\n')
            .map_or(0, |i| i + 1);
        let line_end = self.chars[line_start..]
            .iter()
            .position(|(_, c)| *c == '\n')
            .map_or(self.chars.len(), |i| line_start + i);
        let line: String = self.chars[line_start..line_end]
            .iter()
            .map(|(_, c)| c)
            .collect();
        let indent: String = self.chars[line_start..pos.min(line_end)]
            .iter()
            .map(|(_, c)| if *c == '\t' { '\t' } else { ' ' })
            .collect();
        format!("{line}\n{indent}^")
    }
}
