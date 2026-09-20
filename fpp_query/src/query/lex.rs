//! Tokenizer for the query language.

use crate::Diagnosed;
use fpp_core::{BytePos, SourceFile, Span};
use std::str::Chars;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    /// An identifier, or one of the words the parser treats as an operator
    /// (`in`, `contains`, `starts_with`, `ends_with`, `matches`, `true`,
    /// `false`, `null`).
    Ident(String),
    Int(i64),
    Str(String),
    Dollar,
    At,
    Caret,
    Dot,
    Comma,
    Plus,
    LParen,
    RParen,
    LBracket,
    RBracket,
    EqEq,
    BangEq,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    Bang,
    /// End of input. Carries a zero-length span so "expected X, found end of
    /// query" still points somewhere.
    Eof,
}

impl Tok {
    /// How this token is written, for "expected ..., found ..." messages.
    pub fn describe(&self) -> String {
        match self {
            Tok::Ident(s) => format!("`{s}`"),
            Tok::Int(n) => format!("`{n}`"),
            Tok::Str(s) => format!("string `{s}`"),
            Tok::Eof => "end of query".to_string(),
            other => format!("`{}`", other.punct()),
        }
    }

    fn punct(&self) -> &'static str {
        match self {
            Tok::Dollar => "$",
            Tok::At => "@",
            Tok::Caret => "^",
            Tok::Dot => ".",
            Tok::Comma => ",",
            Tok::Plus => "+",
            Tok::LParen => "(",
            Tok::RParen => ")",
            Tok::LBracket => "[",
            Tok::RBracket => "]",
            Tok::EqEq => "==",
            Tok::BangEq => "!=",
            Tok::Lt => "<",
            Tok::Le => "<=",
            Tok::Gt => ">",
            Tok::Ge => ">=",
            Tok::AndAnd => "&&",
            Tok::OrOr => "||",
            Tok::Bang => "!",
            Tok::Ident(_) | Tok::Int(_) | Tok::Str(_) | Tok::Eof => "",
        }
    }
}

pub struct Lexed {
    pub toks: Vec<Tok>,
    /// `spans[i]` covers `toks[i]`, in the query's own [`SourceFile`].
    pub spans: Vec<Span>,
}

struct Lexer<'a> {
    pos: usize,
    content: &'a str,

    /// Iterator over chars. Slightly faster than a &str.
    chars: Chars<'a>,

    /// The file the query text was read from, and the byte offset within it at
    /// which `content` starts.
    file: SourceFile,
    offset: usize,
}

const EOF_CHAR: char = '\0';

#[inline]
fn is_base_10_digit(ch: char) -> bool {
    ch.is_ascii_digit()
}

/// A digit, or the `_` an integer literal may separate its digits with.
#[inline]
fn is_number_rest(ch: char) -> bool {
    is_base_10_digit(ch) || ch == '_'
}

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

/// Tokenize `text`, which begins at byte `offset` of `file`.
///
/// Emits a diagnostic and returns `Err(Diagnosed)` on the first invalid character
/// or unterminated string.
pub fn lex(file: SourceFile, offset: usize, text: &str) -> Result<Lexed, Diagnosed> {
    let mut lexer = Lexer::new(file, offset, text);
    let mut toks = Vec::new();
    let mut spans = Vec::new();

    while let Some((tok, span)) = lexer.next_token()? {
        toks.push(tok);
        spans.push(span);
    }

    toks.push(Tok::Eof);
    spans.push(lexer.eof_span());
    Ok(Lexed { toks, spans })
}

impl<'a> Lexer<'a> {
    fn new(file: SourceFile, offset: usize, content: &'a str) -> Lexer<'a> {
        let chars = content.chars();
        Lexer {
            pos: 0,
            content,
            chars,
            file,
            offset,
        }
    }

    /// The next token and its span, or `None` at the end of the query.
    /// Whitespace separates tokens but is never one itself.
    fn next_token(&mut self) -> Result<Option<(Tok, Span)>, Diagnosed> {
        self.eat_while(|c| c.is_ascii_whitespace());

        let start = self.pos;
        let Some(first_char) = self.bump() else {
            return Ok(None);
        };

        let tok = match first_char {
            // Operators whose second character is optional, taken greedily so
            // that `<=` never lexes as `<` followed by a stray `=`.
            '<' => {
                if self.eat('=') {
                    Tok::Le
                } else {
                    Tok::Lt
                }
            }

            '>' => {
                if self.eat('=') {
                    Tok::Ge
                } else {
                    Tok::Gt
                }
            }

            '!' => {
                if self.eat('=') {
                    Tok::BangEq
                } else {
                    Tok::Bang
                }
            }

            // Operators that exist only doubled: a lone `=`, `&` or `|` is not a
            // token of this language.
            '=' => {
                self.eat_doubled(start, '=')?;
                Tok::EqEq
            }

            '&' => {
                self.eat_doubled(start, '&')?;
                Tok::AndAnd
            }

            '|' => {
                self.eat_doubled(start, '|')?;
                Tok::OrOr
            }

            // String literal
            '"' | '\'' => self.eat_string_literal(start, first_char)?,

            // Integer literal, and the negative one that `-` can only start
            '0'..='9' => self.eat_number(start)?,
            '-' if is_base_10_digit(self.first()) => self.eat_number(start)?,

            'A'..='Z' | 'a'..='z' | '_' => {
                self.eat_while(is_identifier_rest);
                Tok::Ident(self.content[start..self.pos].to_string())
            }

            '$' => Tok::Dollar,
            '@' => Tok::At,
            '^' => Tok::Caret,
            '.' => Tok::Dot,
            ',' => Tok::Comma,
            '+' => Tok::Plus,
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            '[' => Tok::LBracket,
            ']' => Tok::RBracket,

            c => return Err(self.unexpected_char(start, c)),
        };

        Ok(Some((tok, self.span(start))))
    }

    /// An integer literal, `start` being the offset of its first character: a
    /// digit, or the `-` of a negative literal.
    fn eat_number(&mut self, start: usize) -> Result<Tok, Diagnosed> {
        self.eat_while(is_number_rest);

        let digits: String = self.content[start..self.pos]
            .chars()
            .filter(|c| *c != '_')
            .collect();
        match digits.parse::<i64>() {
            Ok(n) => Ok(Tok::Int(n)),
            Err(_) => Err(self.error(
                start,
                "integer literal does not fit in a 64-bit signed integer",
            )),
        }
    }

    /// A quoted string, `open` being the offset of the `quote` that started it.
    /// Double quotes honour `\\ \" \' \n \t \0`; single quotes are literal
    /// throughout.
    fn eat_string_literal(&mut self, open: usize, quote: char) -> Result<Tok, Diagnosed> {
        let mut out = String::new();
        loop {
            match self.bump() {
                Some(c) if c == quote => return Ok(Tok::Str(out)),

                Some('\\') if quote == '"' => {
                    // The backslash just consumed, so the diagnostic below carets
                    // the escape and not the character after it.
                    let esc_start = self.pos - 1;
                    let Some(esc) = self.bump() else { break };
                    out.push(match esc {
                        '\\' => '\\',
                        '"' => '"',
                        '\'' => '\'',
                        'n' => '\n',
                        't' => '\t',
                        '0' => '\0',
                        _ => {
                            self.span(esc_start)
                                .error(format!("unknown escape `\\{esc}` in string literal"))
                                .note("supported escapes are \\\\ \\\" \\' \\n \\t \\0")
                                .emit();
                            return Err(Diagnosed);
                        }
                    });
                }

                // One whole character, so multi-byte input survives.
                Some(c) => out.push(c),

                None => break,
            }
        }

        Err(self.error(open, "unterminated string literal"))
    }

    /// Consume the second half of an operator that is only ever written doubled.
    fn eat_doubled(&mut self, start: usize, c: char) -> Result<(), Diagnosed> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(self.unexpected_char(start, c))
        }
    }

    /// A span over the token text from `start` to the current position, shifted
    /// into `file`.
    fn span(&self, start: usize) -> Span {
        Span::new(
            self.file,
            BytePos::try_from(self.offset + start).unwrap_or(BytePos::MAX),
            BytePos::try_from(self.pos - start).unwrap_or(0),
            None,
        )
    }

    /// The zero-length span at the end of the query, carried by [`Tok::Eof`].
    fn eof_span(&self) -> Span {
        self.span(self.pos)
    }

    /// Emit an error over the token text from `start` to the current position and
    /// return the marker that ends the lex.
    fn error<T: Into<String>>(&self, start: usize, msg: T) -> Diagnosed {
        self.span(start).error(msg).emit();
        Diagnosed
    }

    /// The character `c` at `start` begins no token of this language.
    fn unexpected_char(&self, start: usize, c: char) -> Diagnosed {
        self.error(start, format!("unexpected character `{c}` in query"))
    }

    fn as_str(&self) -> &'a str {
        self.chars.as_str()
    }

    /// Peeks the next symbol from the input stream without consuming it.
    /// If requested position doesn't exist, `EOF_CHAR` is returned.
    /// However, getting `EOF_CHAR` doesn't always mean actual end of input,
    /// it should be checked with `is_eof` method.
    fn first(&self) -> char {
        // `.next()` optimizes better than `.nth(0)`
        self.chars.clone().next().unwrap_or(EOF_CHAR)
    }

    /// Checks if there is nothing more to consume.
    fn is_eof(&self) -> bool {
        self.as_str().is_empty()
    }

    /// Moves to the next character.
    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        self.pos += c.len_utf8();

        Some(c)
    }

    /// Consumes the next character if it is `c`.
    fn eat(&mut self, c: char) -> bool {
        if self.first() == c {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Eats symbols while predicate returns true or until the end of input is reached.
    fn eat_while(&mut self, mut predicate: impl FnMut(char) -> bool) {
        while predicate(self.first()) && !self.is_eof() {
            self.bump();
        }
    }
}
