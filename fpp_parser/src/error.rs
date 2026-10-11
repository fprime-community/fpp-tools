use fpp_core::{Diagnostic, Level, Span};
use fpp_lexer::TokenKind;
use std::fmt::Formatter;

/// A syntax error
#[derive(Debug)]
pub(crate) enum ParseError {
    /// A token that does not begin any of the alternatives of a production
    ExpectedOneOf {
        expected: Vec<TokenKind>,
        got_kind: TokenKind,
        got_span: Span,
        msg: &'static str,
    },

    /// A token other than the one a production requires
    ExpectedToken {
        expected: TokenKind,
        got: TokenKind,
        last: Span,
        msg: &'static str,
    },

    /// The end of the input where a production requires a token
    UnexpectedEof { last: Span },

    /// A token that does not belong where it occurs
    UnexpectedToken { got_kind: TokenKind, got_span: Span },

    /// The end of the input inside a `construct`, reported at `loc`, where
    /// the alternatives of the construct fail
    EndOfInputIn { loc: Span, construct: &'static str },
}

impl ParseError {
    /// Whether the error is at the end of the input
    pub(crate) fn is_at_end_of_input(&self) -> bool {
        match self {
            ParseError::ExpectedOneOf { got_kind, .. }
            | ParseError::UnexpectedToken { got_kind, .. } => *got_kind == TokenKind::EOF,
            ParseError::ExpectedToken { got, .. } => *got == TokenKind::EOF,
            ParseError::UnexpectedEof { .. } => true,
            ParseError::EndOfInputIn { .. } => false,
        }
    }
}

/// An include error
#[derive(Debug)]
pub(crate) enum IncludeError {
    /// Include cycle, through the files in `paths`: each one includes the
    /// next, and the last is the first
    Cycle { loc: Span, paths: Vec<String> },
}

/// A file error
#[derive(Debug)]
pub(crate) enum FileError {
    /// Cannot open file
    CannotOpen { loc: Span, name: String },
    /// Cannot resolve path
    CannotResolvePath { loc: Span, name: String },
}

struct TokenList(Vec<TokenKind>);
impl std::fmt::Display for TokenList {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for tok in self.0.iter() {
            if !first {
                f.write_str(", ")?;
            }

            tok.fmt(f)?;
            first = false
        }

        Ok(())
    }
}

impl From<ParseError> for Diagnostic {
    fn from(value: ParseError) -> Diagnostic {
        match value {
            ParseError::ExpectedOneOf {
                got_kind,
                got_span,
                msg,
                expected,
            } => Diagnostic::new(got_span, Level::Error, format!("syntax error: {msg}"))
                .note(format!("expected one of {}", TokenList(expected)))
                .note(format!("got {}", got_kind)),
            ParseError::ExpectedToken {
                last,
                msg,
                expected,
                got,
            } => Diagnostic::new(last, Level::Error, format!("syntax error: {msg}"))
                .note(format!("expected {}", expected))
                .note(format!("got {}", got)),
            ParseError::UnexpectedEof { last } => {
                Diagnostic::new(last, Level::Error, "syntax error: unexpected end of input")
            }
            ParseError::UnexpectedToken { got_kind, got_span } => {
                Diagnostic::new(got_span, Level::Error, "syntax error: unexpected token")
                    .note(format!("got {}", got_kind))
            }
            ParseError::EndOfInputIn { loc, construct } => Diagnostic::new(
                loc,
                Level::Error,
                format!("syntax error: unexpected end of input in {construct}"),
            ),
        }
    }
}

impl From<IncludeError> for Diagnostic {
    fn from(value: IncludeError) -> Diagnostic {
        match value {
            IncludeError::Cycle { loc, paths } => {
                let last = paths.len() - 1;
                paths.into_iter().enumerate().fold(
                    Diagnostic::new(loc, Level::Error, "include cycle:"),
                    |diag, (i, path)| match i == last {
                        true => diag.note(path),
                        false => diag.note(format!("{path} includes")),
                    },
                )
            }
        }
    }
}

impl From<FileError> for Diagnostic {
    fn from(value: FileError) -> Diagnostic {
        match value {
            FileError::CannotOpen { loc, name } => {
                Diagnostic::new(loc, Level::Error, format!("cannot open file {name}"))
            }
            FileError::CannotResolvePath { loc, name } => {
                Diagnostic::new(loc, Level::Error, format!("cannot resolve path {name}"))
            }
        }
    }
}

/// The result of a production
pub(crate) type ParseResult<T> = Result<T, ParseError>;
