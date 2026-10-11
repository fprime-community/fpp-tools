use fpp_core::{BytePos, SourceFile, Span, Spanned};
use fpp_lexer::TokenKind;
use std::fmt::Debug;

/// A token of the grammar
#[derive(Debug)]
pub struct Token {
    /// The kind of the token
    pub kind: TokenKind,
    /// The location of the token
    pub span: Span,
    /// The string value of an identifier, annotation or literal
    pub str: Option<String>,
}

impl Token {
    pub fn new(
        kind: TokenKind,
        str: Option<String>,
        file: SourceFile,
        start: BytePos,
        length: BytePos,
        include_span: Option<Span>,
    ) -> Token {
        Token {
            kind,
            span: Span::new(file, start, length, include_span),
            str,
        }
    }

    pub fn kind(&self) -> TokenKind {
        self.kind
    }

    /// The string value of the token, empty if it has none
    pub fn str(&self) -> &str {
        match self.str.as_ref() {
            None => "",
            Some(txt) => txt,
        }
    }
}

impl Spanned for Token {
    fn span(&self) -> Span {
        self.span
    }
}
