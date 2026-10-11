use fpp_core::{Diagnostic, Level, Span};
use fpp_lexer::TokenKind;
use std::fmt::Formatter;

/// A syntax error
#[derive(Debug)]
pub(crate) enum ParseError {
    ExpectedOneOf {
        expected: Vec<TokenKind>,
        got_kind: TokenKind,
        got_span: Span,
        msg: &'static str,
    },

    ExpectedToken {
        expected: TokenKind,
        got: TokenKind,
        last: Span,
        msg: &'static str,
    },

    UnexpectedEof {
        last: Span,
    },

    /// A token that does not belong where it occurs
    UnexpectedToken {
        got_kind: TokenKind,
        got_span: Span,
    },
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
                Diagnostic::new(last, Level::Error, "unexpected end of input")
            }
            ParseError::UnexpectedToken { got_kind, got_span } => {
                Diagnostic::new(got_span, Level::Error, "syntax error: unexpected token")
                    .note(format!("got {}", got_kind))
            }
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

pub(crate) type ParseResult<T> = Result<T, ParseError>;
