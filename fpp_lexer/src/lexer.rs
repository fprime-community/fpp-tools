use crate::token::KeywordKind::*;
use crate::token::TokenKind::*;
use crate::token::{Token, TokenKind};
use std::str::Chars;

pub struct LexerError {
    pub pos: usize,
    pub len: usize,
    pub msg: String,
}

pub struct Lexer<'a> {
    pos: usize,
    indent: u32,
    escaped_identifier: bool,
    content: &'a str,

    /// Iterator over chars. Slightly faster than a &str.
    chars: Chars<'a>,

    errors: Vec<LexerError>,
}

const EOF_CHAR: char = '\0';

/// Verify if a character is within the number base system
#[inline]
fn is_digit_in_base(ch: char, base: i32) -> bool {
    let num = match ch {
        '0'..='9' => (ch as i32) - ('0' as i32),
        'a'..='z' => (ch as i32) - ('a' as i32) + 10,
        'A'..='Z' => (ch as i32) - ('A' as i32) + 10,
        _ => -1,
    };

    0 <= num && num <= base
}

#[inline]
fn is_base_10_digit(ch: char) -> bool {
    ch.is_ascii_digit()
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

/// True if `c` is valid as a non-first character of an identifier.
/// See [Rust language reference](https://doc.rust-lang.org/reference/identifiers.html) for
/// a formal definition of valid identifier name.
pub fn is_id_continue(c: char) -> bool {
    unicode_ident::is_xid_continue(c)
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.reset_token();
        let start = self.pos;

        match self.next_token_kind() {
            EOF => None,
            raw_kind => {
                let end = self.pos;
                let kind = match (raw_kind, self.escaped_identifier) {
                    (Identifier, false) => {
                        // Check if this is a keyword
                        match &self.content[start..end] {
                            "action" => Keyword(Action),
                            "active" => Keyword(Active),
                            "activity" => Keyword(Activity),
                            "always" => Keyword(Always),
                            "array" => Keyword(Array),
                            "assert" => Keyword(Assert),
                            "async" => Keyword(Async),
                            "at" => Keyword(At),
                            "base" => Keyword(Base),
                            "block" => Keyword(Block),
                            "bool" => Keyword(Bool),
                            "change" => Keyword(Change),
                            "command" => Keyword(Command),
                            "component" => Keyword(Component),
                            "connections" => Keyword(Connections),
                            "constant" => Keyword(Constant),
                            "container" => Keyword(Container),
                            "cpu" => Keyword(Cpu),
                            "default" => Keyword(Default),
                            "deployment" => Keyword(Deployment),
                            "diagnostic" => Keyword(Diagnostic),
                            "dictionary" => Keyword(Dictionary),
                            "do" => Keyword(Do),
                            "drop" => Keyword(Drop),
                            "else" => Keyword(Else),
                            "enter" => Keyword(Enter),
                            "entry" => Keyword(Entry),
                            "enum" => Keyword(Enum),
                            "event" => Keyword(Event),
                            "every" => Keyword(Every),
                            "exit" => Keyword(Exit),
                            "external" => Keyword(External),
                            "F32" => Keyword(F32),
                            "F64" => Keyword(F64),
                            "false" => Keyword(False),
                            "fatal" => Keyword(Fatal),
                            "format" => Keyword(Format),
                            "get" => Keyword(Get),
                            "group" => Keyword(Group),
                            "guard" => Keyword(Guard),
                            "guarded" => Keyword(Guarded),
                            "health" => Keyword(Health),
                            "high" => Keyword(High),
                            "hook" => Keyword(Hook),
                            "I16" => Keyword(I16),
                            "I32" => Keyword(I32),
                            "I64" => Keyword(I64),
                            "I8" => Keyword(I8),
                            "id" => Keyword(Id),
                            "if" => Keyword(If),
                            "implements" => Keyword(Implements),
                            "import" => Keyword(Import),
                            "include" => Keyword(Include),
                            "initial" => Keyword(Initial),
                            "input" => Keyword(Input),
                            "instance" => Keyword(Instance),
                            "interface" => Keyword(Interface),
                            "internal" => Keyword(Internal),
                            "choice" => Keyword(Choice),
                            "locate" => Keyword(Locate),
                            "low" => Keyword(Low),
                            "machine" => Keyword(Machine),
                            "match" => Keyword(Match),
                            "module" => Keyword(Module),
                            "omit" => Keyword(Omit),
                            "on" => Keyword(On),
                            "opcode" => Keyword(Opcode),
                            "orange" => Keyword(Orange),
                            "output" => Keyword(Output),
                            "packet" => Keyword(Packet),
                            "packets" => Keyword(Packets),
                            "param" => Keyword(Param),
                            "passive" => Keyword(Passive),
                            "phase" => Keyword(Phase),
                            "port" => Keyword(Port),
                            "priority" => Keyword(Priority),
                            "product" => Keyword(Product),
                            "queue" => Keyword(Queue),
                            "queued" => Keyword(Queued),
                            "record" => Keyword(Record),
                            "recv" => Keyword(Recv),
                            "red" => Keyword(Red),
                            "ref" => Keyword(Ref),
                            "reg" => Keyword(Reg),
                            "request" => Keyword(Request),
                            "resp" => Keyword(Resp),
                            "save" => Keyword(Save),
                            "send" => Keyword(Send),
                            "serial" => Keyword(Serial),
                            "set" => Keyword(Set),
                            "severity" => Keyword(Severity),
                            "signal" => Keyword(Signal),
                            "size" => Keyword(Size),
                            "sizeof" => Keyword(Sizeof),
                            "stack" => Keyword(Stack),
                            "state" => Keyword(State),
                            "string" => Keyword(String_),
                            "struct" => Keyword(Struct),
                            "sync" => Keyword(Sync),
                            "system" => Keyword(System),
                            "telemetry" => Keyword(Telemetry),
                            "text" => Keyword(Text),
                            "throttle" => Keyword(Throttle),
                            "time" => Keyword(Time),
                            "topology" => Keyword(Topology),
                            "true" => Keyword(True),
                            "type" => Keyword(Type),
                            "U16" => Keyword(U16),
                            "U32" => Keyword(U32),
                            "U64" => Keyword(U64),
                            "U8" => Keyword(U8),
                            "unmatched" => Keyword(Unmatched),
                            "update" => Keyword(Update),
                            "warning" => Keyword(Warning),
                            "with" => Keyword(With),
                            "yellow" => Keyword(Yellow),
                            _ => Identifier,
                        }
                    }
                    (Identifier, true) => Identifier,
                    _ => raw_kind,
                };

                Some(Token {
                    kind,
                    len: end - start,
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

    fn error<T: Into<String>>(&mut self, len: usize, msg: T) {
        self.errors.push(LexerError {
            pos: self.pos - len,
            len,
            msg: msg.into(),
        })
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

            // Escaped identifier
            '$' => {
                if is_identifier_first(self.first()) {
                    self.eat_while(is_identifier_rest);
                    self.escaped_identifier = true;
                    Identifier
                } else {
                    Unknown
                }
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

                self.eat_number(base)
            }

            // More numeric literals
            '1'..='9' => self.eat_number(10),

            'A'..='Z' | 'a'..='z' | '_' => {
                self.eat_while(is_identifier_rest);
                Identifier
            }

            // Escaped newline
            '\\' => {
                // Absorb spaces until newline
                self.eat_while(|c| c == ' ');
                if self.first() != '\n' {
                    let start = self.pos;
                    self.eat_while(|c| c != '\n');
                    self.error(
                        self.pos - start,
                        "non whitespace character illegal after line continuation",
                    );
                    self.bump();
                } else {
                    // Skip the newline
                    self.bump();
                }

                Whitespace
            }

            // String literals
            '\"' => {
                match (self.first(), self.second()) {
                    // Multi-line string literal
                    ('\"', '\"') => {
                        // Skip the initial newline if it exists
                        if self.third() == '\n' {
                            self.bump_bytes(3);
                        } else {
                            self.bump_bytes(2);
                        }

                        self.eat_multiline_string_literal()
                    }

                    // Empty string literal
                    ('\"', _) => {
                        self.bump();
                        LiteralString
                    }

                    // String literal
                    _ => self.eat_string_literal(),
                }
            }

            // Fractional floating literal (or DOT)
            '.' => match self.first() {
                '0'..='9' => self.eat_fraction(),
                _ => Dot,
            },

            // Comment
            '#' => {
                self.eat_until(b'\n');
                Comment
            }

            // End of lines
            '\n' => {
                self.eat_while(|c| c == '\n');
                Eol
            }

            // Annotation
            '@' => {
                if self.first() == '<' {
                    self.bump();
                    self.eat_until(b'\n');
                    PostAnnotation
                } else {
                    self.eat_until(b'\n');
                    PreAnnotation
                }
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

            '<' => {
                if self.first() == '<' {
                    self.bump();
                    ShiftLeft
                } else {
                    Unknown
                }
            }

            '>' => {
                if self.first() == '>' {
                    self.bump();
                    ShiftRight
                } else {
                    Unknown
                }
            }

            '+' => Plus,
            '/' => Slash,
            '=' => Equals,
            ';' => Semi,
            ':' => Colon,
            ',' => Comma,
            '(' => LeftParen,
            '{' => LeftCurly,
            '[' => LeftSquare,
            ')' => RightParen,
            ']' => RightSquare,
            '}' => RightCurly,
            c if !c.is_ascii() => self.invalid_ident(),
            _ => Unknown,
        }
    }

    fn eat_multiline_string_literal(&mut self) -> TokenKind {
        // Count the indent on the first line
        self.indent = 0;
        while self.first() == ' ' {
            self.bump();
            self.indent += 1;
        }

        loop {
            match self.bump() {
                // Search for triple quotes "".to_string()"
                Some('\"') => {
                    if self.first() == '\"' && self.second() == '\"' {
                        self.bump_bytes(2);
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
                    self.error(0, "unclosed multi-line string literal");
                    return LiteralMultilineString {
                        indent: self.indent,
                    };
                }
            }
        }
    }

    fn eat_string_literal(&mut self) -> TokenKind {
        loop {
            if self.first() == '\n' {
                self.error(0, "unclosed string literal");
                return LiteralString;
            }

            match self.bump() {
                // Search for triple quotes "".to_string()"
                Some('\"') => {
                    self.indent = 0;
                    return LiteralString;
                }

                Some('\\') => {
                    if self.first() == '\n' {
                        self.error(0, "unclosed string literal");
                        return LiteralString;
                    }

                    // Skip over escaped character
                    self.bump();
                }

                Some(_) => {}

                None => {
                    self.error(0, "unclosed string literal");
                    return LiteralString;
                }
            }
        }
    }

    fn eat_number(&mut self, base: i32) -> TokenKind {
        self.eat_while(|ch| -> bool { is_digit_in_base(ch, base) });

        match self.first() {
            '.' if base == 10 => {
                self.bump();
                self.eat_fraction();
                LiteralFloat
            }

            // Exponential
            'e' | 'E' => self.eat_fraction(),

            _ => LiteralInt,
        }
    }

    fn eat_to_next(&mut self) {
        self.eat_while(is_identifier_rest)
    }

    fn invalid_ident(&mut self) -> TokenKind {
        // Start is already eaten, eat the rest of identifier.
        self.eat_while(|c| {
            const ZERO_WIDTH_JOINER: char = '\u{200d}';
            is_id_continue(c) || (!c.is_ascii()) || c == ZERO_WIDTH_JOINER
        });
        Unknown
    }

    fn eat_fraction(&mut self) -> TokenKind {
        self.eat_while(is_base_10_digit);

        match (self.first(), self.second()) {
            ('e' | 'E', '0'..='9') => {
                self.bump();
                self.eat_while(is_base_10_digit);
                if is_identifier_first(self.first()) {
                    self.error(0, "invalid literal number");
                    self.eat_to_next();
                }

                LiteralFloat
            }

            (c, _) if is_identifier_first(c) => {
                self.error(0, "invalid literal number");
                self.eat_to_next();
                LiteralFloat
            }

            _ => LiteralFloat,
        }
    }

    pub fn as_str(&self) -> &'a str {
        self.chars.as_str()
    }

    /// Peeks the next symbol from the input stream without consuming it.
    /// If requested position doesn't exist, `EOF_CHAR` is returned.
    /// However, getting `EOF_CHAR` doesn't always mean actual end of file,
    /// it should be checked with `is_eof` method.
    pub fn first(&self) -> char {
        // `.next()` optimizes better than `.nth(0)`
        self.chars.clone().next().unwrap_or(EOF_CHAR)
    }

    /// Peeks the second symbol from the input stream without consuming it.
    pub fn second(&self) -> char {
        // `.next()` optimizes better than `.nth(1)`
        let mut iter = self.chars.clone();
        iter.next();
        iter.next().unwrap_or(EOF_CHAR)
    }

    /// Peeks the third symbol from the input stream without consuming it.
    pub fn third(&self) -> char {
        // `.next()` optimizes better than `.nth(1)`
        let mut iter = self.chars.clone();
        iter.next();
        iter.next();
        iter.next().unwrap_or(EOF_CHAR)
    }

    /// Checks if there is nothing more to consume.
    pub fn is_eof(&self) -> bool {
        self.chars.as_str().is_empty()
    }

    /// Resets the number of bytes consumed to 0.
    fn reset_token(&mut self) {
        self.indent = 0;
        self.escaped_identifier = false;
    }

    /// Moves to the next character.
    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        self.pos += c.len_utf8();

        Some(c)
    }

    /// Moves to a substring by a number of bytes.
    fn bump_bytes(&mut self, n: usize) {
        self.chars = self.as_str()[n..].chars();
        self.pos += n;
    }

    /// Eats symbols while predicate returns true or until the end of file is reached.
    fn eat_while(&mut self, mut predicate: impl FnMut(char) -> bool) {
        // It was tried making optimized version of this for e.g. line comments, but
        // LLVM can inline all of this and compile it down to fast iteration over bytes.
        while predicate(self.first()) && !self.is_eof() {
            self.bump();
        }
    }

    fn eat_until(&mut self, byte: u8) {
        let b = self.as_str().as_bytes();
        self.chars = match memchr::memchr(byte, b) {
            Some(index) => {
                self.pos += index;
                self.as_str()[index..].chars()
            }
            None => {
                self.pos += b.len();
                "".chars()
            }
        }
    }
}
