use crate::error::{ParseError, ParseResult};
use crate::token::Token;
use fpp_core::{BytePos, Diagnostic, Level, SourceFile, Span, Spanned};
use fpp_lexer::{Lexer, LexerError, TokenKind};
use std::collections::VecDeque;

/// The cursor over the tokens of a file.
///
/// The cursor pulls the tokens of `fpp_lexer` through the rules for newlines:
/// it drops whitespace and comments, folds a run of newlines into one
/// end-of-line token, drops the newlines after a token that eats them and
/// before a closing token, and gives each token its string value.
pub struct Cursor<'a> {
    /// The tokens the lexer produced for the whole file that have not been
    /// pulled through the cursor
    tokens: VecDeque<fpp_lexer::Token>,

    /// The lexer errors that have not been emitted
    errors: Vec<LexerError>,

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
    /// Lex the whole file and emit its lexer errors, so that they come before
    /// any syntax error in the file
    pub(crate) fn new(
        source_file: SourceFile,
        content: &'a str,
        include_span: Option<Span>,
    ) -> Cursor<'a> {
        let mut lexer = Lexer::new(content);
        let tokens = lexer.by_ref().collect();
        let errors = lexer.errors().cloned().collect();

        let mut cursor = Cursor {
            tokens,
            errors,
            pos: 0,
            content,
            file: source_file,
            lookaheads: Default::default(),
            last_consumed_span: Span::new(source_file, 0, 0, include_span),
            include_span,
            lookahead: None,
        };

        cursor.emit_errors();
        cursor
    }

    /// Emit the lexer errors that have not been emitted
    pub fn emit_errors(&mut self) {
        for err in std::mem::take(&mut self.errors) {
            let diagnostic = Diagnostic::new(
                Span::new(
                    self.file,
                    err.pos as BytePos,
                    err.len as BytePos,
                    self.include_span,
                ),
                Level::Error,
                format!("invalid token: {}", err.msg),
            );

            err.notes
                .into_iter()
                .fold(diagnostic, |diagnostic, note| diagnostic.note(note))
                .emit();
        }
    }

    /// Read through all whitespace, comments, and newlines until another token is reached
    /// This does not update any state of the current token
    /// Return the token we reached (or None if EOF)
    fn eat_newlines(&mut self) -> Option<fpp_lexer::Token> {
        loop {
            let token = self.tokens.pop_front()?;
            match token.kind {
                // Skip whitespace (a line continuation is whitespace that ends the run)
                TokenKind::Eol => {
                    self.pos += token.len;
                }
                TokenKind::Whitespace if !self.content[self.pos..].starts_with('\\') => {
                    self.pos += token.len;
                }
                // Skip comments
                TokenKind::Comment => {
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
                None => self.tokens.pop_front()?,
                Some(lookahead) => lookahead,
            };

            self.pos += tok.len;
            match tok.kind {
                TokenKind::EOF => unreachable!(),
                // The lexer reported the invalid token
                TokenKind::Unknown => {}
                TokenKind::Whitespace => {}
                // Newline (or line comment)
                TokenKind::Eol | TokenKind::Comment => {
                    // Check the next token
                    // Some tokens eat newlines and others don't
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
                                    // Fetch the next token to avoid this newline
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
                        Some(java_trim(&self.content[prev + 2..prev + tok.len]).to_string()),
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
                        Some(java_trim(&self.content[prev + 1..prev + tok.len]).to_string()),
                        self.file,
                        prev as BytePos,
                        tok.len as BytePos,
                        self.include_span,
                    ));
                }

                TokenKind::LiteralString => {
                    let raw = &self.content[prev + 1..prev + tok.len];
                    let raw = raw.strip_suffix('"').unwrap_or(raw);

                    return Some(Token::new(
                        tok.kind,
                        Some(string_lit_single_value(raw)),
                        self.file,
                        prev as BytePos,
                        tok.len as BytePos,
                        self.include_span,
                    ));
                }

                TokenKind::LiteralMultilineString { indent } => {
                    let raw = &self.content[prev + 3..prev + tok.len];
                    let raw = raw.strip_suffix("\"\"\"").unwrap_or(raw);

                    return Some(Token::new(
                        tok.kind,
                        Some(string_lit_multi_value(raw, indent)),
                        self.file,
                        prev as BytePos,
                        tok.len as BytePos,
                        self.include_span,
                    ));
                }

                // Tokens that do not absorb newlines after and have text
                TokenKind::LiteralInt => {
                    let text = &self.content[prev..(prev + tok.len)];
                    let text = match text.strip_prefix("0X") {
                        Some(digits) => format!("0x{digits}"),
                        None => text.to_string(),
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

                TokenKind::LiteralFloat => {
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
                | TokenKind::ShiftLeft
                | TokenKind::ShiftRight => {
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
                | TokenKind::RightArrow
                | TokenKind::Plus
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

    /// The location of the next token, or the end of the input if there is none.
    /// An end-of-line token, which spans the run of newlines and comments it
    /// stands for, is located where the run starts.
    fn next_token_span(&mut self) -> Span {
        let file = self.file;
        let include_span = self.include_span;
        match self.peek_internal(0) {
            Some(tok) if tok.kind() == TokenKind::Eol => {
                Span::new(file, tok.span().start().pos(), 0, include_span)
            }
            Some(tok) => tok.span(),
            None => self.end_of_input_span(),
        }
    }

    /// The empty span at the end of the input
    fn end_of_input_span(&self) -> Span {
        Span::new(
            self.file,
            self.content.len() as BytePos,
            0,
            self.include_span,
        )
    }

    /// Generate a new error while expecting a certain type of token
    /// Messages here are meant to only be simple literals, the full error message
    /// will be formatted given other context information.
    /// The error is located at the next token.
    pub fn err_expected_token(
        &mut self,
        msg: &'static str,
        expected: TokenKind,
        got: TokenKind,
    ) -> ParseError {
        ParseError::ExpectedToken {
            expected,
            got,
            last: self.next_token_span(),
            msg,
        }
    }

    pub fn err_unexpected_eof(&self) -> ParseError {
        ParseError::UnexpectedEof {
            last: self.end_of_input_span(),
        }
    }

    pub fn err_expected_one_of(
        &mut self,
        msg: &'static str,
        expected_one_of: Vec<TokenKind>,
    ) -> ParseError {
        let (got_kind, got_span) = match self.peek_internal(0) {
            None => (TokenKind::EOF, self.end_of_input_span()),
            Some(got) => (got.kind, got.span),
        };

        ParseError::ExpectedOneOf {
            expected: expected_one_of,
            got_span,
            got_kind,
            msg,
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

/// Remove the leading and trailing characters at or below the space character,
/// the way Java's `String.trim` does
fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

/// Append the character `ch` that follows a backslash in a string literal
fn put_string_escape_char(out: &mut String, ch: char) {
    match ch {
        '\\' => out.push('\\'),
        '"' => out.push('"'),
        // TODO(tumbar) We could put in more string escape sequences
        _ => out.push(ch),
    }
}

/// The value of a single-line string literal whose text between the quotes
/// is `raw`
fn string_lit_single_value(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                if let Some(ch) = chars.next() {
                    put_string_escape_char(&mut out, ch);
                }
            }
            _ => out.push(ch),
        }
    }

    out
}

/// The value of a multi-line string literal whose text between the quotes is
/// `raw` and whose first line has an initial space indent of `full_indent`.
/// Each line loses at most `full_indent` leading spaces.
fn string_lit_multi_value(raw: &str, full_indent: u32) -> String {
    // Skip the initial newline if it exists
    let raw = raw
        .strip_prefix("\r\n")
        .or_else(|| raw.strip_prefix('\n'))
        .unwrap_or(raw);

    // Skip the initial space indent
    let raw = raw.get(full_indent as usize..).unwrap_or("");

    let mut out = String::with_capacity(raw.len());
    let mut skip = 0;
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            ' ' => {
                if skip == 0 {
                    out.push(' ');
                } else {
                    skip -= 1;
                }
            }
            '\r' if chars.peek() == Some(&'\n') => {
                chars.next();
                out.push('\n');
                skip = full_indent;
            }
            '\n' => {
                out.push('\n');
                skip = full_indent;
            }
            '\\' => {
                match chars.next() {
                    Some('\r') if chars.peek() == Some(&'\n') => {
                        chars.next();
                        put_string_escape_char(&mut out, '\n');
                    }
                    Some(ch) => put_string_escape_char(&mut out, ch),
                    None => {}
                }
                skip = 0;
            }
            _ => {
                out.push(ch);
                skip = 0;
            }
        }
    }

    out
}

#[cfg(test)]
mod lexer_spec;

#[cfg(test)]
mod test {
    use crate::cursor::Cursor;
    use crate::error::ParseError;
    use crate::token::Token;
    use fpp_core::{SourceFile, Spanned};
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

    /// The kinds and texts of the tokens of `content`
    fn lex_texts(content: &str) -> Vec<(TokenKind, String)> {
        lex(content)
            .iter()
            .map(|tok| (tok.kind(), tok.text().to_string()))
            .collect()
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
        assert_token_eq(&tokens[idx.next()], LiteralString, "string \"");

        assert_token_eq(
            &tokens[idx.next()],
            LiteralMultilineString { indent: 4 },
            "a multiline literal string with \"\"\" some escapes \" \"\n",
        );
        assert_token_eq(
            &tokens[idx.next()],
            LiteralMultilineString { indent: 0 },
            "",
        );
        assert_eq!(tokens.len(), 12);
    }

    #[test]
    fn escaped_strings() {
        assert_eq!(
            lex_texts(r#""\"" "\\" "\\\"" "\"\\" "x\qy" "i\nc.fppi" "{\}""#),
            vec![
                (LiteralString, "\"".to_string()),
                (LiteralString, "\\".to_string()),
                (LiteralString, "\\\"".to_string()),
                (LiteralString, "\"\\".to_string()),
                (LiteralString, "xqy".to_string()),
                (LiteralString, "inc.fppi".to_string()),
                (LiteralString, "{}".to_string()),
            ]
        );
    }

    #[test]
    fn multiline_string_indent() {
        // Each line loses at most the indent of the first line, and only spaces
        let tokens = lex_texts(
            "\"\"\"\n    abc\n  de\n      deep\n\t b\n    x\\\"y\n  \\\n   z\n    \"\"\"",
        );
        assert_eq!(
            tokens,
            vec![(
                LiteralMultilineString { indent: 4 },
                "abc\nde\n  deep\n\t b\nx\"y\n\n   z\n".to_string()
            )]
        );

        // The first line need not start on a new line
        assert_eq!(
            lex_texts("\"\"\"  abc\n    def\n  \"\"\""),
            vec![(
                LiteralMultilineString { indent: 2 },
                "abc\n  def\n".to_string()
            )]
        );
    }

    #[test]
    fn multiline_string_non_ascii() {
        assert_eq!(
            lex_texts("\"\"\"\n    x\nabc\u{e9}\n   \u{e9}\u{e9}\n    \"\"\""),
            vec![(
                LiteralMultilineString { indent: 4 },
                "x\nabc\u{e9}\n\u{e9}\u{e9}\n".to_string()
            )]
        );
    }

    #[test]
    fn crlf() {
        assert_eq!(
            lex_texts("a = 1\r\n# c\r\nb \\\r\n  c\r\n@ d\r\n\"\"\"\r\n  x\r\n  y\r\n  \"\"\"\r\n"),
            vec![
                (Identifier, "a".to_string()),
                (Equals, String::new()),
                (LiteralInt, "1".to_string()),
                (Eol, String::new()),
                (Identifier, "b".to_string()),
                (Identifier, "c".to_string()),
                (Eol, String::new()),
                (PreAnnotation, "d".to_string()),
                (LiteralMultilineString { indent: 2 }, "x\ny\n".to_string()),
                (Eol, String::new()),
            ]
        );
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
    fn annotations_at_end_of_input() {
        assert_eq!(lex_texts("@ x"), vec![(PreAnnotation, "x".to_string())]);
        assert_eq!(
            lex_texts("constant a = 1\n@"),
            vec![
                (Keyword(KeywordKind::Constant), String::new()),
                (Identifier, "a".to_string()),
                (Equals, String::new()),
                (LiteralInt, "1".to_string()),
                (Eol, String::new()),
                (PreAnnotation, String::new()),
            ]
        );
        assert_eq!(
            lex_texts("a @<"),
            vec![
                (Identifier, "a".to_string()),
                (PostAnnotation, String::new())
            ]
        );
    }

    #[test]
    fn annotation_trim() {
        // Trimming removes the characters at or below the space character
        assert_eq!(
            lex_texts("@ abc\u{a0}\n@ \u{1}def\u{1}\n"),
            vec![
                (PreAnnotation, "abc\u{a0}".to_string()),
                (PreAnnotation, "def".to_string()),
            ]
        );
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
    fn hex_prefix() {
        assert_eq!(
            lex_texts("0X1F 0x1F"),
            vec![
                (LiteralInt, "0x1F".to_string()),
                (LiteralInt, "0x1F".to_string()),
            ]
        );
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
    fn escape_newline_before_closing_token() {
        // A line continuation ends a run of newlines, so the newline before it is
        // kept
        assert_eq!(
            lex_texts("(1\n\\\n)"),
            vec![
                (LeftParen, String::new()),
                (LiteralInt, "1".to_string()),
                (Eol, String::new()),
                (RightParen, String::new()),
            ]
        );
    }

    #[test]
    fn invalid_tokens() {
        let tokens = lex(r#"1ee 	 $ 1e1e "
    ""#);
        assert_eq!(tokens.len(), 7);
        let mut idx = Index(0);
        assert_token_eq(&tokens[idx.next()], LiteralFloat, "1");
        assert_token_eq(&tokens[idx.next()], Identifier, "ee");
        assert_token_eq(&tokens[idx.next()], LiteralFloat, "1e1");
        assert_token_eq(&tokens[idx.next()], Identifier, "e");
        assert_token_eq(&tokens[idx.next()], LiteralString, "");
        assert_token_eq(&tokens[idx.next()], Eol, "");
        assert_token_eq(&tokens[idx.next()], LiteralString, "");

        let tokens = lex(r#"""" asdaldkasl"#);
        assert_eq!(tokens.len(), 1);
        assert_token_eq(
            &tokens[0],
            LiteralMultilineString { indent: 1 },
            "asdaldkasl",
        );
    }

    #[test]
    fn unclosed_non_ascii_strings() {
        assert_eq!(
            lex_texts("\"caf\u{e9}\nconstant"),
            vec![
                (LiteralString, "caf\u{e9}".to_string()),
                (Eol, String::new()),
                (Keyword(KeywordKind::Constant), String::new()),
            ]
        );
        assert_eq!(
            lex_texts("\"\"\"\u{e9}\u{e9}\u{e9}"),
            vec![(
                LiteralMultilineString { indent: 0 },
                "\u{e9}\u{e9}\u{e9}".to_string()
            )]
        );
    }

    #[test]
    fn escape_newline_error() {
        let tokens = lex(r#"escaped \hello
    newline"#);
        assert_eq!(tokens.len(), 2);
        let mut idx = Index(0);
        assert_token_eq(&tokens[idx.next()], Identifier, "escaped");
        assert_token_eq(&tokens[idx.next()], Identifier, "newline");
    }

    #[test]
    fn symbols() {
        let tokens = lex(r#": . ,

        = ()

        ) {}

        } []

        ] - + ; / *

        1

        )1

        }1

        ]"#);

        assert_eq!(tokens.len(), 24);
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

    #[test]
    fn newline_suppression() {
        // `->` absorbs the newlines after it; `<<` and `>>` do not
        let kinds = |s: &str| lex(s).iter().map(|t| t.kind()).collect::<Vec<_>>();
        assert_eq!(kinds("a ->\n  b"), vec![Identifier, RightArrow, Identifier]);
        assert_eq!(
            kinds("1 <<\n  2"),
            vec![LiteralInt, ShiftLeft, Eol, LiteralInt]
        );
        assert_eq!(
            kinds("1 >>\n  2"),
            vec![LiteralInt, ShiftRight, Eol, LiteralInt]
        );
    }

    /// Run `f` on a cursor over `content`
    fn with_cursor<T>(content: &str, f: impl FnOnce(&mut Cursor) -> T) -> T {
        let mut diagnostics_str = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics_str));

        fpp_core::run(&mut ctx, || {
            let file = SourceFile::new("<stdin>", content.to_string());
            let mut cursor = Cursor::new(file, content, None);
            f(&mut cursor)
        })
    }

    #[test]
    fn errors_are_located_at_the_next_token() {
        with_cursor("a b", |cursor| {
            let b = cursor.peek_span(1).unwrap();
            cursor.next();

            match cursor.consume(Equals) {
                Err(ParseError::ExpectedToken { last, got, .. }) => {
                    assert_eq!(last, b);
                    assert_eq!(got, Identifier);
                }
                other => panic!("{other:?}"),
            }
        });

        // Before the first token
        with_cursor("# comment\n\nb", |cursor| {
            let b = cursor.peek_span(1).unwrap();
            assert_eq!(cursor.peek(1), Identifier);

            match cursor.consume(Identifier) {
                Err(ParseError::ExpectedToken { last, got, .. }) => {
                    assert_eq!(got, Eol);
                    assert_eq!(last.start().line(), 0);
                }
                other => panic!("{other:?}"),
            }

            cursor.next();
            assert_eq!(cursor.next().unwrap().span(), b);
        });
    }

    #[test]
    fn end_of_input_errors_keep_the_expectation() {
        with_cursor("a", |cursor| {
            cursor.next();

            match cursor.err_expected_one_of("expression expected", vec![LiteralInt]) {
                ParseError::ExpectedOneOf {
                    got_kind,
                    got_span,
                    msg,
                    expected,
                } => {
                    assert_eq!(got_kind, EOF);
                    assert_eq!(msg, "expression expected");
                    assert_eq!(expected, vec![LiteralInt]);
                    assert_eq!(got_span.start().pos(), 1);
                    assert!(got_span.is_empty());
                }
                other => panic!("{other:?}"),
            }
        });
    }
}
