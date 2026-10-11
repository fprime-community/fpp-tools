use crate::error::{ParseError, ParseResult};
use crate::token::Token;
use fpp_core::{BytePos, Diagnostic, Level, SourceFile, Span, Spanned};
use fpp_lexer::{Lexer, TokenKind};
use std::collections::VecDeque;

pub struct Cursor<'a> {
    lexer: Lexer<'a>,

    /// The lexer only tells us the length of the next tokens
    /// We need to track the current position in the file
    pos: usize,

    /// File contents
    content: &'a str,

    /// The source file we are parsing
    file: SourceFile,

    /// The span of the `include` that brought in this file
    include_span: Option<Span>,

    /// Parsing lookaheads which are ready to be consumed
    lookaheads: VecDeque<Token>,

    /// Lexing lookahead which still needs to be pulled through the cursor
    lookahead: Option<fpp_lexer::Token>,

    /// Tracks the last consumed span for easily building spans across many tokens
    last_consumed_span: Span,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(
        source_file: SourceFile,
        content: &'a str,
        include_span: Option<Span>,
    ) -> Cursor<'a> {
        Cursor {
            lexer: Lexer::new(content),
            pos: 0,
            content,
            file: source_file,
            lookaheads: Default::default(),
            last_consumed_span: Span::new(source_file, 1, 0, None),
            include_span,
            lookahead: None,
        }
    }

    pub fn emit_errors(&self) {
        self.lexer.errors().for_each(|err| {
            Diagnostic::new(
                Span::new(
                    self.file,
                    err.pos as BytePos,
                    err.len as BytePos,
                    self.include_span,
                ),
                Level::Error,
                "syntax error: invalid token",
            )
            .annotation(err.msg.clone())
            .emit();
        })
    }

    /// Keep eating whitespace/comments/newlines until we reach another token
    /// Return the token we reached (or None if EOF)
    fn eat_newlines(&mut self) -> Option<fpp_lexer::Token> {
        loop {
            let token = self.lexer.next()?;
            match token.kind {
                TokenKind::Eol | TokenKind::Comment | TokenKind::Whitespace => {
                    self.pos += token.len;
                }
                _ => return Some(token),
            }
        }
    }

    fn next_internal(&mut self) -> Option<Token> {
        loop {
            let prev = self.pos;
            let tok = match self.lookahead.take() {
                None => self.lexer.next()?,
                Some(lookahead) => lookahead,
            };

            self.pos += tok.len;
            match tok.kind {
                TokenKind::EOF => unreachable!(),
                TokenKind::Unknown => {
                    Diagnostic::new(
                        Span::new(
                            self.file,
                            prev as BytePos,
                            tok.len as BytePos,
                            self.include_span,
                        ),
                        Level::Error,
                        format!(
                            "syntax error: invalid character {:#?}",
                            self.content.as_bytes()[prev] as char
                        ),
                    )
                    .emit();
                }
                TokenKind::Whitespace => {}
                TokenKind::Eol | TokenKind::Comment => {
                    // Check what comes after the EOL to see if this actually an EOL or
                    // just whitespace
                    return match self.eat_newlines() {
                        None => Some(Token::new(
                            TokenKind::Eol,
                            None,
                            self.file,
                            prev as BytePos,
                            (self.pos - prev) as BytePos,
                            self.include_span,
                        )),
                        Some(lookahead) => {
                            match lookahead.kind {
                                TokenKind::RightParen
                                | TokenKind::RightCurly
                                | TokenKind::RightSquare => {
                                    // Absorb all the previous whitespace before these closing tokens
                                    let lookahead_prev = self.pos;
                                    self.pos += lookahead.len;
                                    Some(Token::new(
                                        lookahead.kind,
                                        None,
                                        self.file,
                                        lookahead_prev as BytePos,
                                        lookahead.len as BytePos,
                                        self.include_span,
                                    ))
                                }
                                _ => {
                                    // Save this other type of token for later
                                    self.lookahead = Some(lookahead);

                                    // This is actually a newline delimiter
                                    Some(Token::new(
                                        TokenKind::Eol,
                                        None,
                                        self.file,
                                        prev as BytePos,
                                        (self.pos - prev) as BytePos,
                                        self.include_span,
                                    ))
                                }
                            }
                        }
                    };
                }
                TokenKind::Identifier => {
                    // Check if this identifier is an escaped keyword
                    let start = if self.content.as_bytes()[prev] == b'$' {
                        prev + 1
                    } else {
                        prev
                    };

                    return Some(Token::new(
                        TokenKind::Identifier,
                        Some(self.content[start..self.pos].to_string()),
                        self.file,
                        prev as BytePos,
                        tok.len as BytePos,
                        self.include_span,
                    ));
                }
                TokenKind::PostAnnotation => {
                    self.lookahead = self.eat_newlines();
                    return Some(Token::new(
                        tok.kind,
                        Some(
                            self.content[prev + 2..=(prev + tok.len - 1)]
                                .trim()
                                .to_string(),
                        ),
                        self.file,
                        prev as BytePos,
                        tok.len as BytePos,
                        self.include_span,
                    ));
                }

                TokenKind::PreAnnotation => {
                    self.lookahead = self.eat_newlines();
                    return Some(Token::new(
                        tok.kind,
                        Some(self.content[prev + 1..=prev + tok.len].trim().to_string()),
                        self.file,
                        prev as BytePos,
                        tok.len as BytePos,
                        self.include_span,
                    ));
                }

                TokenKind::LiteralString => {
                    let text = if tok.len >= 2 {
                        self.content[prev + 1..(prev + 1 + tok.len - 2)].to_string()
                    } else {
                        "".to_string()
                    };

                    return Some(Token::new(
                        tok.kind,
                        Some(text),
                        self.file,
                        prev as BytePos,
                        tok.len as BytePos,
                        self.include_span,
                    ));
                }

                TokenKind::LiteralMultilineString { indent } => {
                    let text = if tok.len >= 6 {
                        let raw_text = self.content[prev + 3..(prev + 3 + tok.len - 6)].to_string();
                        let lines: Vec<_> = raw_text
                            .split('\n')
                            .map(|l| {
                                if l.len() > indent as usize {
                                    l[(indent as usize)..].to_string()
                                } else {
                                    "".to_string()
                                }
                            })
                            .collect();
                        lines.join("\n")
                    } else {
                        "".to_string()
                    };

                    return Some(Token::new(
                        tok.kind,
                        Some(text),
                        self.file,
                        prev as BytePos,
                        tok.len as BytePos,
                        self.include_span,
                    ));
                }

                // Tokens that do not absorb newlines after and have text
                TokenKind::LiteralFloat | TokenKind::LiteralInt => {
                    return Some(Token::new(
                        tok.kind,
                        Some(self.content[prev..(prev + tok.len)].to_string()),
                        self.file,
                        prev as BytePos,
                        tok.len as BytePos,
                        self.include_span,
                    ));
                }

                // Tokens that do not absorb newlines after (and do not have text)
                TokenKind::Keyword(_)
                | TokenKind::Dot
                | TokenKind::RightParen
                | TokenKind::RightCurly
                | TokenKind::RightSquare
                | TokenKind::RightArrow => {
                    return Some(Token::new(
                        tok.kind,
                        None,
                        self.file,
                        prev as BytePos,
                        tok.len as BytePos,
                        self.include_span,
                    ));
                }

                // Tokens that eat the newlines after them
                TokenKind::Star
                | TokenKind::Slash
                | TokenKind::Minus
                | TokenKind::Plus
                | TokenKind::ShiftLeft
                | TokenKind::ShiftRight
                | TokenKind::Equals
                | TokenKind::Semi
                | TokenKind::Comma
                | TokenKind::Colon
                | TokenKind::LeftParen
                | TokenKind::LeftCurly
                | TokenKind::LeftSquare => {
                    self.lookahead = self.eat_newlines();
                    return Some(Token::new(
                        tok.kind,
                        None,
                        self.file,
                        prev as BytePos,
                        tok.len as BytePos,
                        self.include_span,
                    ));
                }
            }
        }
    }

    fn peek_internal(&mut self, n: usize) -> Option<&Token> {
        if self.lookaheads.len() > n {
            Some(self.lookaheads.get(n).unwrap())
        } else {
            // Queue up as many tokens as we need
            while self.lookaheads.len() <= n {
                let tok = self.next_internal()?;
                self.lookaheads.push_back(tok);
            }

            Some(self.lookaheads.get(n).unwrap())
        }
    }

    pub fn peek_span(&mut self, n: usize) -> Option<Span> {
        self.peek_internal(n).map(|tok| tok.span())
    }

    /// Look ahead 'n' tokens and get the token kind
    /// This will pull in tokens from the lexer when needed
    pub fn peek(&mut self, n: usize) -> TokenKind {
        match self.peek_internal(n) {
            Some(tok) => tok.kind(),
            _ => TokenKind::EOF,
        }
    }

    pub fn last_token_span(&self) -> Span {
        self.last_consumed_span
    }

    /// Generate a new error while expecting a certain type of token
    /// Messages here are meant to only be simple literals, the full error message
    /// will be formatted given other context information.
    pub fn err_expected_token(
        &self,
        msg: &'static str,
        expected: TokenKind,
        got: TokenKind,
    ) -> ParseError {
        ParseError::ExpectedToken {
            expected,
            got,
            last: self.last_consumed_span,
            msg,
        }
    }

    pub fn err_unexpected_eof(&self) -> ParseError {
        ParseError::UnexpectedEof {
            last: self.last_consumed_span,
        }
    }

    // Insert a single token into the front of the queue to be pulled next
    // pub fn insert(&mut self, token: Token) {
    //     self.token_queue.push_front(token)
    // }

    pub fn err_expected_one_of(
        &mut self,
        msg: &'static str,
        expected_one_of: Vec<TokenKind>,
    ) -> ParseError {
        match self.peek_internal(0) {
            None => self.err_unexpected_eof(),
            Some(got) => ParseError::ExpectedOneOf {
                expected: expected_one_of,
                got_span: got.span,
                got_kind: got.kind,
                msg,
            },
        }
    }

    /// Consume the next token in the stream
    /// Returns None if EOF has been reached
    pub fn next(&mut self) -> Option<Token> {
        // Try to pull token off the queue
        let tok = match self.lookaheads.pop_front() {
            // No more tokens in our queue, go to the lexer
            None => self.next_internal(),
            Some(tok) => Some(tok),
        };

        match tok {
            Some(tok) => {
                self.last_consumed_span = tok.span();
                Some(tok)
            }
            None => None,
        }
    }

    #[inline]
    pub fn consume(&mut self, kind: TokenKind) -> ParseResult<Token> {
        let current = self.peek(0);
        if current == kind {
            Ok(self.next().unwrap())
        } else {
            Err(self.err_expected_token("unexpected token", kind, current))
        }
    }
}

#[cfg(test)]
mod test {
    use crate::cursor::Cursor;
    use crate::token::Token;
    use fpp_core::SourceFile;
    use fpp_lexer::TokenKind::*;
    use fpp_lexer::{KeywordKind, TokenKind};

    struct Index(usize);

    impl Index {
        fn next(&mut self) -> usize {
            let out = self.0;
            self.0 += 1;
            out
        }
    }

    fn lex(content: &str) -> Vec<Token> {
        let mut diagnostics_str = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics_str));

        fpp_core::run(&mut ctx, || {
            let file = SourceFile::new("<stdin>", content.to_string());
            let mut cursor = Cursor::new(file, content, None);

            let mut out = vec![];
            loop {
                match cursor.next() {
                    None => break,
                    Some(tok) => {
                        out.push(tok);
                    }
                }
            }

            out
        })
    }

    fn assert_token_eq(token: &Token, kind: TokenKind, text: &str) {
        assert_eq!(token.kind(), kind);
        assert_eq!(token.text(), text);
    }

    #[test]
    fn skip_whitespace() {
        let tokens = lex("   ");
        assert_eq!(tokens.len(), 0)
    }

    #[test]
    fn comment() {
        let tokens = lex(r#" # comment

    # more comments

    "#);
        assert_token_eq(&tokens[0], Eol, "");
        assert_eq!(tokens.len(), 1);
    }

    #[test]
    fn eat_newlines() {
        let tokens = lex(r#"
    
    "#);
        assert_eq!(tokens.len(), 1);
        let mut idx = Index(0);
        assert_token_eq(&tokens[idx.next()], Eol, "");
    }

    #[test]
    fn literals() {
        let tokens = lex(
            r#"12 1.23 0x10 0x1AEF 001 1e30 10.3e3 .3e3 "" "string \"" """
    a multiline literal string with \"\"\" some \escapes " "
    """ """""" "#,
        );
        let mut idx = Index(0);
        assert_token_eq(&tokens[idx.next()], LiteralInt, "12");
        assert_token_eq(&tokens[idx.next()], LiteralFloat, "1.23");
        assert_token_eq(&tokens[idx.next()], LiteralInt, "0x10");
        assert_token_eq(&tokens[idx.next()], LiteralInt, "0x1AEF");
        assert_token_eq(&tokens[idx.next()], LiteralInt, "001");
        assert_token_eq(&tokens[idx.next()], LiteralFloat, "1e30");
        assert_token_eq(&tokens[idx.next()], LiteralFloat, "10.3e3");
        assert_token_eq(&tokens[idx.next()], LiteralFloat, ".3e3");
        assert_token_eq(&tokens[idx.next()], LiteralString, "");
        assert_token_eq(&tokens[idx.next()], LiteralString, "string \\\"");

        assert_token_eq(
            &tokens[idx.next()],
            LiteralMultilineString { indent: 4 },
            "\na multiline literal string with \\\"\\\"\\\" some \\escapes \" \"\n",
        );
        assert_token_eq(
            &tokens[idx.next()],
            LiteralMultilineString { indent: 0 },
            "",
        );
        assert_eq!(tokens.len(), 12);
    }

    #[test]
    fn annotations() {
        let tokens = lex(r#"@ Pre annotation
        Some Identifiers @< Post annotation"#);

        assert_eq!(tokens.len(), 4);
        let mut idx = Index(0);
        assert_token_eq(&tokens[idx.next()], PreAnnotation, "Pre annotation");
        assert_token_eq(&tokens[idx.next()], Identifier, "Some");
        assert_token_eq(&tokens[idx.next()], Identifier, "Identifiers");
        assert_token_eq(&tokens[idx.next()], PostAnnotation, "Post annotation");
    }

    #[test]
    fn identifiers_and_keywords() {
        let tokens =
            lex(r#"Ident _underscope_start with_numbers01_asd yellow $yellow action every"#);

        assert_eq!(tokens.len(), 7);
        let mut idx = Index(0);
        assert_token_eq(&tokens[idx.next()], Identifier, "Ident");
        assert_token_eq(&tokens[idx.next()], Identifier, "_underscope_start");
        assert_token_eq(&tokens[idx.next()], Identifier, "with_numbers01_asd");
        assert_token_eq(&tokens[idx.next()], Keyword(KeywordKind::Yellow), "");
        assert_token_eq(&tokens[idx.next()], Identifier, "yellow");
        assert_token_eq(&tokens[idx.next()], Keyword(KeywordKind::Action), "");
        assert_token_eq(&tokens[idx.next()], Keyword(KeywordKind::Every), "");
    }

    #[test]
    fn escape_newline() {
        let tokens = lex(r#"escaped \
    newline"#);
        assert_eq!(tokens.len(), 2);
        let mut idx = Index(0);
        assert_token_eq(&tokens[idx.next()], Identifier, "escaped");
        assert_token_eq(&tokens[idx.next()], Identifier, "newline");
    }

    #[test]
    fn invalid_tokens() {
        let tokens = lex(r#"1ee 	 $ 1e1e "
    ""#);
        assert_eq!(tokens.len(), 5);
        let mut idx = Index(0);
        assert_token_eq(&tokens[idx.next()], LiteralFloat, "1ee");
        assert_token_eq(&tokens[idx.next()], LiteralFloat, "1e1e");
        assert_token_eq(&tokens[idx.next()], LiteralString, "");
        assert_token_eq(&tokens[idx.next()], Eol, "");
        assert_token_eq(&tokens[idx.next()], LiteralString, "");

        let tokens = lex(r#"""" asdaldkasl"#);
        assert_eq!(tokens.len(), 1);
        // assert_token_eq(&tokens[0], Error("unclosed multi-line string literal"), "");
    }

    #[test]
    fn escape_newline_error() {
        let tokens = lex(r#"escaped \hello
    newline"#);
        assert_eq!(tokens.len(), 2);
        let mut idx = Index(0);
        assert_token_eq(&tokens[idx.next()], Identifier, "escaped");
        // assert_token_eq(
        //     &tokens[idx.next()],
        //     Error("Non whitespace character illegal after line continuation"),
        //     "",
        // );
        assert_token_eq(&tokens[idx.next()], Identifier, "newline");
    }

    #[test]
    fn symbols() {
        let tokens = lex(r#": . ,

        = ()

        ) {}

        } []

        ] -> - + ; / *

        1

        )1

        }1

        ]"#);

        assert_eq!(tokens.len(), 25);
        let mut idx = Index(0);
        assert_token_eq(&tokens[idx.next()], Colon, "");
        assert_token_eq(&tokens[idx.next()], Dot, "");
        assert_token_eq(&tokens[idx.next()], Comma, "");
        assert_token_eq(&tokens[idx.next()], Equals, "");
        assert_token_eq(&tokens[idx.next()], LeftParen, "");
        assert_token_eq(&tokens[idx.next()], RightParen, "");
        assert_token_eq(&tokens[idx.next()], RightParen, "");
        assert_token_eq(&tokens[idx.next()], LeftCurly, "");
        assert_token_eq(&tokens[idx.next()], RightCurly, "");
        assert_token_eq(&tokens[idx.next()], RightCurly, "");
        assert_token_eq(&tokens[idx.next()], LeftSquare, "");
        assert_token_eq(&tokens[idx.next()], RightSquare, "");
        assert_token_eq(&tokens[idx.next()], RightSquare, "");
        assert_token_eq(&tokens[idx.next()], RightArrow, "");
        assert_token_eq(&tokens[idx.next()], Minus, "");
        assert_token_eq(&tokens[idx.next()], Plus, "");
        assert_token_eq(&tokens[idx.next()], Semi, "");
        assert_token_eq(&tokens[idx.next()], Slash, "");
        assert_token_eq(&tokens[idx.next()], Star, "");
        assert_token_eq(&tokens[idx.next()], LiteralInt, "1");
        assert_token_eq(&tokens[idx.next()], RightParen, "");
        assert_token_eq(&tokens[idx.next()], LiteralInt, "1");
        assert_token_eq(&tokens[idx.next()], RightCurly, "");
        assert_token_eq(&tokens[idx.next()], LiteralInt, "1");
        assert_token_eq(&tokens[idx.next()], RightSquare, "");
    }
}
