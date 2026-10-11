use fpp_core::{Diagnostic, Level, Span};
use fpp_lexer::TokenKind;
use std::fmt::Formatter;

/// A syntax error
#[derive(Debug)]
pub(crate) enum SyntaxError {
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
        got_span: Span,
        msg: &'static str,
    },

    /// The end of the input where a production requires a token
    UnexpectedEof { loc: Span },

    /// A token that does not belong where it occurs
    UnexpectedToken { got_kind: TokenKind, got_span: Span },

    /// The end of the input inside a `construct`, reported at `loc`, where
    /// the alternatives of the construct fail
    EndOfInputIn { loc: Span, construct: &'static str },
}

impl SyntaxError {
    /// Whether the error is at the end of the input
    pub(crate) fn is_at_end_of_input(&self) -> bool {
        match self {
            SyntaxError::ExpectedOneOf { got_kind, .. }
            | SyntaxError::UnexpectedToken { got_kind, .. } => *got_kind == TokenKind::EOF,
            SyntaxError::ExpectedToken { got, .. } => *got == TokenKind::EOF,
            SyntaxError::UnexpectedEof { .. } => true,
            SyntaxError::EndOfInputIn { .. } => false,
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

impl From<SyntaxError> for Diagnostic {
    fn from(value: SyntaxError) -> Diagnostic {
        match value {
            SyntaxError::ExpectedOneOf {
                got_kind,
                got_span,
                msg,
                expected,
            } => Diagnostic::new(got_span, Level::Error, format!("syntax error: {msg}"))
                .note(format!("expected one of {}", TokenList(expected)))
                .note(format!("got {}", got_kind)),
            SyntaxError::ExpectedToken {
                got_span,
                msg,
                expected,
                got,
            } => Diagnostic::new(got_span, Level::Error, format!("syntax error: {msg}"))
                .note(format!("expected {}", expected))
                .note(format!("got {}", got)),
            SyntaxError::UnexpectedEof { loc } => {
                Diagnostic::new(loc, Level::Error, "syntax error: unexpected end of input")
            }
            SyntaxError::UnexpectedToken { got_kind, got_span } => {
                Diagnostic::new(got_span, Level::Error, "syntax error: unexpected token")
                    .note(format!("got {}", got_kind))
            }
            SyntaxError::EndOfInputIn { loc, construct } => Diagnostic::new(
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
pub(crate) type ParseResult<T> = Result<T, SyntaxError>;
