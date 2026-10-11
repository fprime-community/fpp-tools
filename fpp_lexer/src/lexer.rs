use crate::chars::{CR, LF, digit2int, is_identifier_part};
use crate::token::TokenKind::*;
use crate::token::{KeywordKind, Token, TokenKind};
use std::str::Chars;

/// A lexer error: an invalid token
#[derive(Debug, Clone)]
pub struct LexerError {
    /// The byte offset where the error starts
    pub pos: usize,
    /// The number of bytes the error covers
    pub len: usize,
    /// The error message
    pub msg: String,
    /// Notes that follow the message
    pub notes: Vec<String>,
}

pub struct Lexer<'a> {
    /// The byte offset of the next character to read
    pos: usize,
    /// The byte offset of the first character of the current token
    offset: usize,
    /// The initial space indent of the current multi-line string literal
    indent: u32,
    /// The next identifier is forced to not be a keyword
    escaped_identifier: bool,
    content: &'a str,

    /// The characters that remain to be read
    chars: Chars<'a>,

    errors: Vec<LexerError>,
}

const EOF_CHAR: char = '\0';

#[inline]
fn is_identifier_first(c: char) -> bool {
    matches!(c, 'A'..='Z' | '_' | 'a'..='z')
}

#[inline]
fn is_identifier_rest(c: char) -> bool {
    match c {
        c if is_identifier_first(c) => true,
        '0'..='9' => true,
        _ => false,
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.reset_token();

        match self.next_token_kind() {
            EOF => None,
            raw_kind => {
                let kind = match raw_kind {
                    Identifier => self.finish_ident(),
                    _ => raw_kind,
                };

                Some(Token {
                    kind,
                    len: self.pos - self.offset,
                })
            }
        }
    }
}

impl<'a> Lexer<'a> {
    pub fn new(content: &'a str) -> Lexer<'a> {
        let chars = content.chars();
        Lexer {
            pos: 0,
            offset: 0,
            indent: 0,
            content,
            chars,
            escaped_identifier: false,
            errors: vec![],
        }
    }

    pub fn errors(&self) -> impl Iterator<Item = &LexerError> {
        self.errors.iter()
    }

    /// Generate an error covering the current token
    fn error<T: Into<String>>(&mut self, msg: T) {
        self.error_at(self.offset, self.pos - self.offset, msg, vec![])
    }

    /// Generate an error covering `len` bytes at `pos`
    fn error_at<T: Into<String>>(&mut self, pos: usize, len: usize, msg: T, notes: Vec<String>) {
        self.errors.push(LexerError {
            pos,
            len,
            msg: msg.into(),
            notes,
        })
    }

    /// Finish up construction of identifier
    fn finish_ident(&self) -> TokenKind {
        if self.escaped_identifier {
            Identifier
        } else {
            match KeywordKind::get(&self.content[self.offset..self.pos]) {
                Some(keyword) => Keyword(keyword),
                None => Identifier,
            }
        }
    }

    fn next_token_kind(&mut self) -> TokenKind {
        let first_char = match self.bump() {
            Some(c) => c,
            None => return EOF,
        };

        match first_char {
            // Skip whitespace
            ' ' => {
                self.eat_while(|c| c == ' ');
                Whitespace
            }

            '$' => {
                // Reserved words (identifier that overlaps a keyword)
                if is_identifier_first(self.first()) {
                    self.escaped_identifier = true;
                    self.eat_while(is_identifier_rest);
                    Identifier
                } else {
                    self.error("invalid usage of '$', expected identifier");
                    Unknown
                }
            }

            // Identifier or keyword
            'A'..='Z' | 'a'..='z' | '_' => {
                self.eat_while(is_identifier_rest);
                Identifier
            }

            // Numeric literal
            '0' => {
                let base = match self.first() {
                    'x' | 'X' => {
                        self.bump();
                        16
                    }
                    _ => 10,
                };

                if base != 10 && digit2int(self.first(), base).is_none() {
                    self.error("invalid literal number");
                }

                self.eat_number(base)
            }

            // More numeric literals
            '1'..='9' => self.eat_number(10),

            // Escaped newline
            '\\' => {
                // Absorb spaces until newline
                self.eat_while(|c| c == ' ');
                if self.first() != LF {
                    let start = self.pos;
                    self.eat_while(|c| c != LF);
                    if start < self.pos {
                        // Point at the characters that follow the backslash
                        self.error_at(
                            start,
                            self.pos - start,
                            "expected line continuation",
                            vec![],
                        );
                    } else {
                        // The input ends after the backslash
                        self.error("expected line continuation");
                    }
                }

                // Skip the newline
                self.bump();
                Whitespace
            }

            // String literals
            '"' => {
                if self.first() == '"' {
                    if self.second() == '"' {
                        self.bump();
                        self.bump();

                        // Skip the initial newline if it exists
                        if self.first() == LF {
                            self.bump();
                        }

                        self.indent = self.fetch_string_lit_multi_indent();
                        self.fetch_string_lit_multi()
                    } else {
                        // Empty string literal
                        self.bump();
                        LiteralString
                    }
                } else {
                    self.fetch_string_lit_single()
                }
            }

            // Fractional floating literal (or DOT)
            '.' => {
                // Check if this is actually part of a float
                if self.first().is_ascii_digit() {
                    self.eat_fraction()
                } else {
                    Dot
                }
            }

            // Comment
            '#' => {
                self.eat_while(|c| c != LF);
                Comment
            }

            // End of lines
            LF => {
                self.eat_while(|c| c == LF);
                Eol
            }

            // Annotation
            '@' => {
                let kind = if self.first() == '<' {
                    self.bump();
                    PostAnnotation
                } else {
                    PreAnnotation
                };

                self.eat_while(|c| c != LF);
                kind
            }

            '*' => Star,

            '-' => {
                if self.first() == '>' {
                    self.bump();
                    RightArrow
                } else {
                    Minus
                }
            }

            '+' => Plus,
            '/' => Slash,

            '<' => {
                if self.first() == '<' {
                    self.bump();
                    ShiftLeft
                } else {
                    self.error("'<'");
                    Unknown
                }
            }

            '>' => {
                if self.first() == '>' {
                    self.bump();
                    ShiftRight
                } else {
                    self.error("'>'");
                    Unknown
                }
            }

            '=' => Equals,
            ';' => Semi,
            ':' => Colon,
            ',' => Comma,
            '(' => LeftParen,
            '{' => LeftCurly,
            ')' => RightParen,
            '}' => RightCurly,
            '[' => LeftSquare,
            ']' => RightSquare,

            ch => {
                // Print out the unicode value, in case the console can't
                // display it as a character
                let msg = format!("unicode value {}, hex 0x{:x}", ch as u32, ch as u32);

                // Generate a special error message for tab characters
                // These are especially tricky and also somewhat common
                let notes = if ch == '\t' {
                    vec![
                        "embedded tab characters are not allowed in FPP source files".to_string(),
                        "try configuring your editor to convert tabs to spaces".to_string(),
                    ]
                } else {
                    vec![]
                };

                self.error_at(self.offset, self.pos - self.offset, msg, notes);
                Unknown
            }
        }
    }

    /// Compute the initial space indent
    fn fetch_string_lit_multi_indent(&mut self) -> u32 {
        let mut indent = 0;
        while self.first() == ' ' {
            self.bump();
            indent += 1;
        }

        indent
    }

    fn fetch_string_lit_multi(&mut self) -> TokenKind {
        loop {
            match self.bump() {
                // Search for the closing triple quote
                Some('"') => {
                    if self.first() == '"' && self.second() == '"' {
                        self.bump();
                        self.bump();
                        return LiteralMultilineString {
                            indent: self.indent,
                        };
                    }
                }

                Some('\\') => {
                    // Skip over escaped character
                    self.bump();
                }

                Some(_) => {}

                None => {
                    // The literal runs to the end of the input; report it at its
                    // opening quotes
                    self.error_at(self.offset, 3, "unclosed multi-line string literal", vec![]);
                    return LiteralMultilineString {
                        indent: self.indent,
                    };
                }
            }
        }
    }

    fn fetch_string_lit_single(&mut self) -> TokenKind {
        loop {
            // A single-line string may contain neither a newline nor an escaped
            // newline
            if self.first() == LF || self.is_eof() {
                self.error("missing string double quote closure");
                return LiteralString;
            }

            match self.bump() {
                // Search for the closing quote
                Some('"') => return LiteralString,

                Some('\\') => {
                    if self.first() == LF || self.is_eof() {
                        self.error("missing string double quote closure");
                        return LiteralString;
                    }

                    // Skip over escaped character
                    self.bump();
                }

                _ => {}
            }
        }
    }

    /// Read a number in the given base
    fn eat_number(&mut self, base: u32) -> TokenKind {
        self.eat_while(|ch| digit2int(ch, base).is_some());

        match self.first() {
            '.' if base == 10 => {
                self.bump();
                self.eat_fraction()
            }

            // Exponential
            'e' | 'E' if base == 10 => self.eat_fraction(),

            _ => LiteralInt,
        }
    }

    /// Read the fractional part and exponent of a floating point number if one is
    /// present
    fn eat_fraction(&mut self) -> TokenKind {
        self.eat_while(|c| c.is_ascii_digit());

        if matches!(self.first(), 'e' | 'E') {
            let lookahead = match self.second() {
                '+' | '-' => self.third(),
                c => c,
            };

            if lookahead.is_ascii_digit() {
                self.bump();
                if matches!(self.first(), '+' | '-') {
                    self.bump();
                }

                self.eat_while(|c| c.is_ascii_digit());
            }
        }

        self.check_no_letter();
        LiteralFloat
    }

    fn check_no_letter(&mut self) {
        let ch = self.first();
        if is_identifier_part(ch) && ch >= ' ' && !self.is_eof() {
            // Cover the literal and the letter that ends it
            self.error_at(
                self.offset,
                self.pos - self.offset + ch.len_utf8(),
                "invalid literal number",
                vec![],
            );
        }
    }

    pub fn as_str(&self) -> &'a str {
        self.chars.as_str()
    }

    /// Peeks the character `n` positions ahead without consuming anything,
    /// reducing CR;LF pairs to just LF.
    /// If requested position doesn't exist, `EOF_CHAR` is returned.
    fn peek_char(&self, n: usize) -> char {
        let mut chars = self.chars.clone();
        let mut ch = EOF_CHAR;
        for _ in 0..=n {
            ch = match chars.next() {
                None => return EOF_CHAR,
                Some(CR) if chars.as_str().starts_with(LF) => {
                    chars.next();
                    LF
                }
                Some(c) => c,
            }
        }

        ch
    }

    /// Peeks the next symbol from the input stream without consuming it.
    /// If requested position doesn't exist, `EOF_CHAR` is returned.
    /// However, getting `EOF_CHAR` doesn't always mean actual end of file,
    /// it should be checked with `is_eof` method.
    pub fn first(&self) -> char {
        self.peek_char(0)
    }

    /// Peeks the second symbol from the input stream without consuming it.
    pub fn second(&self) -> char {
        self.peek_char(1)
    }

    /// Peeks the third symbol from the input stream without consuming it.
    pub fn third(&self) -> char {
        self.peek_char(2)
    }

    /// Checks if there is nothing more to consume.
    pub fn is_eof(&self) -> bool {
        self.chars.as_str().is_empty()
    }

    /// Reset the per-token state
    fn reset_token(&mut self) {
        self.offset = self.pos;
        self.indent = 0;
        self.escaped_identifier = false;
    }

    /// Advance one character; reducing CR;LF pairs to just LF
    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        self.pos += c.len_utf8();

        if c == CR && self.chars.as_str().starts_with(LF) {
            self.chars.next();
            self.pos += LF.len_utf8();
            return Some(LF);
        }

        Some(c)
    }

    /// Eats symbols while predicate returns true or until the end of file is reached.
    fn eat_while(&mut self, mut predicate: impl FnMut(char) -> bool) {
        while predicate(self.first()) && !self.is_eof() {
            self.bump();
        }
    }
}
