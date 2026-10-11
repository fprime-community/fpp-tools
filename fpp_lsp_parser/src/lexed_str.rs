//! Lexing `&str` into a sequence of FPP LSP tokens.
//!
//! Note that these tokens, unlike the tokens we feed into the parser, do
//! include info about comments and whitespace.

use crate::SyntaxKind::{self, *};
use fpp_lexer::{KeywordKind, TokenKind};
use std::ops;

pub struct LexedStr<'a> {
    text: &'a str,
    kind: Vec<SyntaxKind>,
    start: Vec<u32>,
    error: Vec<LexError>,
}

struct LexError {
    msg: String,
    token: u32,
}

impl<'a> LexedStr<'a> {
    pub fn new(text: &'a str) -> LexedStr<'a> {
        let mut conv = Converter::new(text);

        let mut lexer = fpp_lexer::Lexer::new(text);
        for token in lexer.by_ref() {
            conv.push(token.kind.into(), token.len);
        }

        let mut res = conv.finalize_with_eof();
        for err in lexer.errors() {
            let token = res.token_at_error(err.pos, err.len);
            let mut msg = format!("invalid token: {}", err.msg);
            if !err.notes.is_empty() {
                msg.push_str("\nnote: ");
                msg.push_str(&err.notes.join("\n"));
            }
            res.error.push(LexError {
                msg,
                token: token as u32,
            });
        }
        res.error.sort_by_key(|e| e.token);
        res
    }

    /// The index of the token that was being lexed when the lexer reported an
    /// error at byte offset `pos` covering `len` bytes. A zero-length error
    /// sits at the end of the token that reported it.
    fn token_at_error(&self, pos: usize, len: usize) -> usize {
        let n = self.len();
        let after = if len == 0 {
            self.start[..n].partition_point(|&s| (s as usize) < pos)
        } else {
            self.start[..n].partition_point(|&s| (s as usize) <= pos)
        };
        after.saturating_sub(1).min(n.saturating_sub(1))
    }

    pub fn as_str(&self) -> &str {
        self.text
    }

    pub fn len(&self) -> usize {
        self.kind.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn kind(&self, i: usize) -> SyntaxKind {
        assert!(i < self.len());
        self.kind[i]
    }

    pub fn text(&self, i: usize) -> &str {
        self.range_text(i..i + 1)
    }

    pub fn range_text(&self, r: ops::Range<usize>) -> &str {
        assert!(r.start < r.end && r.end <= self.len());
        let lo = self.start[r.start] as usize;
        let hi = self.start[r.end] as usize;
        &self.text[lo..hi]
    }

    // Naming is hard.
    pub fn text_range(&self, i: usize) -> ops::Range<usize> {
        assert!(i < self.len());
        let lo = self.start[i] as usize;
        let hi = self.start[i + 1] as usize;
        lo..hi
    }
    pub fn text_start(&self, i: usize) -> usize {
        assert!(i <= self.len());
        self.start[i] as usize
    }
    pub fn text_len(&self, i: usize) -> usize {
        assert!(i < self.len());
        let r = self.text_range(i);
        r.end - r.start
    }

    pub fn error(&self, i: usize) -> Option<&str> {
        assert!(i < self.len());
        let err = self
            .error
            .binary_search_by_key(&(i as u32), |i| i.token)
            .ok()?;
        Some(self.error[err].msg.as_str())
    }

    pub fn errors(&self) -> impl Iterator<Item = (usize, &str)> + '_ {
        self.error
            .iter()
            .map(|it| (it.token as usize, it.msg.as_str()))
    }

    fn push(&mut self, kind: SyntaxKind, offset: usize) {
        self.kind.push(kind);
        self.start.push(offset as u32);
    }
}

struct Converter<'a> {
    res: LexedStr<'a>,
    offset: usize,
}

impl<'a> Converter<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            res: LexedStr {
                text,
                kind: Vec::new(),
                start: Vec::new(),
                error: Vec::new(),
            },
            offset: 0,
        }
    }

    fn finalize_with_eof(mut self) -> LexedStr<'a> {
        self.res.push(EOF, self.offset);
        self.res
    }

    fn push(&mut self, kind: SyntaxKind, len: usize) {
        self.res.push(kind, self.offset);
        self.offset += len;
    }
}

impl From<TokenKind> for SyntaxKind {
    fn from(value: TokenKind) -> Self {
        match value {
            TokenKind::EOF => EOF,
            TokenKind::Whitespace => WHITESPACE,
            TokenKind::Comment => COMMENT,
            TokenKind::Unknown => UNKNOWN,
            TokenKind::Identifier => IDENT,
            TokenKind::PostAnnotation => POST_ANNOTATION,
            TokenKind::PreAnnotation => PRE_ANNOTATION,
            TokenKind::LiteralFloat => LITERAL_FLOAT,
            TokenKind::LiteralInt => LITERAL_INT,
            TokenKind::LiteralString => LITERAL_STRING,
            TokenKind::LiteralMultilineString { .. } => LITERAL_STRING,
            TokenKind::Keyword(keyword) => match keyword {
                KeywordKind::Action => ACTION_KW,
                KeywordKind::Active => ACTIVE_KW,
                KeywordKind::Activity => ACTIVITY_KW,
                KeywordKind::Always => ALWAYS_KW,
                KeywordKind::Array => ARRAY_KW,
                KeywordKind::Assert => ASSERT_KW,
                KeywordKind::Async => ASYNC_KW,
                KeywordKind::At => AT_KW,
                KeywordKind::Base => BASE_KW,
                KeywordKind::Block => BLOCK_KW,
                KeywordKind::Bool => BOOL_KW,
                KeywordKind::Change => CHANGE_KW,
                KeywordKind::Command => COMMAND_KW,
                KeywordKind::Component => COMPONENT_KW,
                KeywordKind::Connections => CONNECTIONS_KW,
                KeywordKind::Constant => CONSTANT_KW,
                KeywordKind::Container => CONTAINER_KW,
                KeywordKind::Cpu => CPU_KW,
                KeywordKind::Default => DEFAULT_KW,
                KeywordKind::Deployment => DEPLOYMENT_KW,
                KeywordKind::Diagnostic => DIAGNOSTIC_KW,
                KeywordKind::Dictionary => DICTIONARY_KW,
                KeywordKind::Do => DO_KW,
                KeywordKind::Drop => DROP_KW,
                KeywordKind::Else => ELSE_KW,
                KeywordKind::Enter => ENTER_KW,
                KeywordKind::Entry => ENTRY_KW,
                KeywordKind::Enum => ENUM_KW,
                KeywordKind::Event => EVENT_KW,
                KeywordKind::Every => EVERY_KW,
                KeywordKind::Exit => EXIT_KW,
                KeywordKind::Expand => EXPAND_KW,
                KeywordKind::External => EXTERNAL_KW,
                KeywordKind::F32 => F32_KW,
                KeywordKind::F64 => F64_KW,
                KeywordKind::False => FALSE_KW,
                KeywordKind::Fatal => FATAL_KW,
                KeywordKind::Format => FORMAT_KW,
                KeywordKind::Get => GET_KW,
                KeywordKind::Group => GROUP_KW,
                KeywordKind::Guard => GUARD_KW,
                KeywordKind::Guarded => GUARDED_KW,
                KeywordKind::Health => HEALTH_KW,
                KeywordKind::High => HIGH_KW,
                KeywordKind::Hook => HOOK_KW,
                KeywordKind::I16 => I16_KW,
                KeywordKind::I32 => I32_KW,
                KeywordKind::I64 => I64_KW,
                KeywordKind::I8 => I8_KW,
                KeywordKind::Id => ID_KW,
                KeywordKind::If => IF_KW,
                KeywordKind::Implements => IMPLEMENTS_KW,
                KeywordKind::Import => IMPORT_KW,
                KeywordKind::Include => INCLUDE_KW,
                KeywordKind::Initial => INITIAL_KW,
                KeywordKind::Input => INPUT_KW,
                KeywordKind::Instance => INSTANCE_KW,
                KeywordKind::Interface => INTERFACE_KW,
                KeywordKind::Internal => INTERNAL_KW,
                KeywordKind::Choice => CHOICE_KW,
                KeywordKind::Locate => LOCATE_KW,
                KeywordKind::Low => LOW_KW,
                KeywordKind::Machine => MACHINE_KW,
                KeywordKind::Match => MATCH_KW,
                KeywordKind::Module => MODULE_KW,
                KeywordKind::Omit => OMIT_KW,
                KeywordKind::On => ON_KW,
                KeywordKind::Opcode => OPCODE_KW,
                KeywordKind::Orange => ORANGE_KW,
                KeywordKind::Output => OUTPUT_KW,
                KeywordKind::Packet => PACKET_KW,
                KeywordKind::Packets => PACKETS_KW,
                KeywordKind::Param => PARAM_KW,
                KeywordKind::Passive => PASSIVE_KW,
                KeywordKind::Phase => PHASE_KW,
                KeywordKind::Port => PORT_KW,
                KeywordKind::Priority => PRIORITY_KW,
                KeywordKind::Product => PRODUCT_KW,
                KeywordKind::Queue => QUEUE_KW,
                KeywordKind::Queued => QUEUED_KW,
                KeywordKind::Record => RECORD_KW,
                KeywordKind::Recv => RECV_KW,
                KeywordKind::Red => RED_KW,
                KeywordKind::Ref => REF_KW,
                KeywordKind::Reg => REG_KW,
                KeywordKind::Request => REQUEST_KW,
                KeywordKind::Resp => RESP_KW,
                KeywordKind::Save => SAVE_KW,
                KeywordKind::Send => SEND_KW,
                KeywordKind::Serial => SERIAL_KW,
                KeywordKind::Set => SET_KW,
                KeywordKind::Severity => SEVERITY_KW,
                KeywordKind::Signal => SIGNAL_KW,
                KeywordKind::Size => SIZE_KW,
                KeywordKind::Sizeof => SIZEOF_KW,
                KeywordKind::Stack => STACK_KW,
                KeywordKind::State => STATE_KW,
                KeywordKind::String_ => STRING_KW,
                KeywordKind::Struct => STRUCT_KW,
                KeywordKind::Sync => SYNC_KW,
                KeywordKind::System => SYSTEM_KW,
                KeywordKind::Telemetry => TELEMETRY_KW,
                KeywordKind::Template => TEMPLATE_KW,
                KeywordKind::Text => TEXT_KW,
                KeywordKind::Throttle => THROTTLE_KW,
                KeywordKind::Time => TIME_KW,
                KeywordKind::Topology => TOPOLOGY_KW,
                KeywordKind::True => TRUE_KW,
                KeywordKind::Type => TYPE_KW,
                KeywordKind::U16 => U16_KW,
                KeywordKind::U32 => U32_KW,
                KeywordKind::U64 => U64_KW,
                KeywordKind::U8 => U8_KW,
                KeywordKind::Unmatched => UNMATCHED_KW,
                KeywordKind::Update => UPDATE_KW,
                KeywordKind::Warning => WARNING_KW,
                KeywordKind::With => WITH_KW,
                KeywordKind::Yellow => YELLOW_KW,
            },
            TokenKind::Colon => COLON,
            TokenKind::Comma => COMMA,
            TokenKind::Dot => DOT,
            TokenKind::Eol => EOL,
            TokenKind::Equals => EQUALS,
            TokenKind::LeftParen => LEFT_PAREN,
            TokenKind::LeftCurly => LEFT_CURLY,
            TokenKind::LeftSquare => LEFT_SQUARE,
            TokenKind::RightParen => RIGHT_PAREN,
            TokenKind::RightCurly => RIGHT_CURLY,
            TokenKind::RightSquare => RIGHT_SQUARE,
            TokenKind::RightArrow => RIGHT_ARROW,
            TokenKind::Minus => MINUS,
            TokenKind::Plus => PLUS,
            TokenKind::Semi => SEMI,
            TokenKind::Slash => SLASH,
            TokenKind::Star => STAR,
            TokenKind::ShiftLeft => SHIFT_LEFT,
            TokenKind::ShiftRight => SHIFT_RIGHT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LexedStr;
    use crate::SyntaxKind::UNKNOWN;

    /// Each error of the lexer is reported once, on the token it was reported
    /// for, including the characters that cannot begin a token
    #[test]
    fn each_lexer_error_is_reported_once() {
        let text = "\t $ \u{2122}\u{2122} < > % 1.5x \"abc\n";
        let mut lexer = fpp_lexer::Lexer::new(text);
        lexer.by_ref().for_each(drop);
        // Each error of this input starts where the token it is reported on
        // starts
        let expected: Vec<(usize, String)> = lexer
            .errors()
            .map(|e| {
                let mut msg = format!("invalid token: {}", e.msg);
                if !e.notes.is_empty() {
                    msg.push_str("\nnote: ");
                    msg.push_str(&e.notes.join("\n"));
                }
                (e.pos, msg)
            })
            .collect();

        let lexed = LexedStr::new(text);
        let errors: Vec<(usize, String)> = lexed
            .errors()
            .map(|(token, msg)| (lexed.text_start(token), msg.to_string()))
            .collect();
        assert_eq!(errors, expected);

        let unknown: Vec<usize> = (0..lexed.len())
            .filter(|&i| lexed.kind(i) == UNKNOWN)
            .collect();
        assert_eq!(unknown.len(), 7);
        for i in unknown {
            assert_eq!(
                lexed.errors().filter(|&(token, _)| token == i).count(),
                1,
                "{:?}",
                lexed.text(i)
            );
        }
    }
}
