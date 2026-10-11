/// A keyword
///
/// The variant for the keyword `string` is `String_`, so that it does not
/// shadow `String` where the variants are imported.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum KeywordKind {
    Action,
    Active,
    Activity,
    Always,
    Array,
    Assert,
    Async,
    At,
    Base,
    Block,
    Bool,
    Change,
    Choice,
    Command,
    Component,
    Connections,
    Constant,
    Container,
    Cpu,
    Default,
    Deployment,
    Diagnostic,
    Dictionary,
    Do,
    Drop,
    Else,
    Enter,
    Entry,
    Enum,
    Event,
    Every,
    Exit,
    Expand,
    External,
    F32,
    F64,
    False,
    Fatal,
    Format,
    Get,
    Group,
    Guard,
    Guarded,
    Health,
    High,
    Hook,
    I16,
    I32,
    I64,
    I8,
    Id,
    If,
    Implements,
    Import,
    Include,
    Initial,
    Input,
    Instance,
    Interface,
    Internal,
    Locate,
    Low,
    Machine,
    Match,
    Module,
    Omit,
    On,
    Opcode,
    Orange,
    Output,
    Packet,
    Packets,
    Param,
    Passive,
    Phase,
    Port,
    Priority,
    Product,
    Queue,
    Queued,
    Record,
    Recv,
    Red,
    Ref,
    Reg,
    Request,
    Resp,
    Save,
    Send,
    Serial,
    Set,
    Severity,
    Signal,
    Size,
    Sizeof,
    Stack,
    State,
    String_,
    Struct,
    Sync,
    System,
    Telemetry,
    Template,
    Text,
    Throttle,
    Time,
    Topology,
    True,
    Type,
    U16,
    U32,
    U64,
    U8,
    Unmatched,
    Update,
    Warning,
    With,
    Yellow,
}

/// The keywords and their spellings, sorted by spelling
pub(crate) const KEYWORDS: [(&str, KeywordKind); 118] = [
    ("F32", KeywordKind::F32),
    ("F64", KeywordKind::F64),
    ("I16", KeywordKind::I16),
    ("I32", KeywordKind::I32),
    ("I64", KeywordKind::I64),
    ("I8", KeywordKind::I8),
    ("U16", KeywordKind::U16),
    ("U32", KeywordKind::U32),
    ("U64", KeywordKind::U64),
    ("U8", KeywordKind::U8),
    ("action", KeywordKind::Action),
    ("active", KeywordKind::Active),
    ("activity", KeywordKind::Activity),
    ("always", KeywordKind::Always),
    ("array", KeywordKind::Array),
    ("assert", KeywordKind::Assert),
    ("async", KeywordKind::Async),
    ("at", KeywordKind::At),
    ("base", KeywordKind::Base),
    ("block", KeywordKind::Block),
    ("bool", KeywordKind::Bool),
    ("change", KeywordKind::Change),
    ("choice", KeywordKind::Choice),
    ("command", KeywordKind::Command),
    ("component", KeywordKind::Component),
    ("connections", KeywordKind::Connections),
    ("constant", KeywordKind::Constant),
    ("container", KeywordKind::Container),
    ("cpu", KeywordKind::Cpu),
    ("default", KeywordKind::Default),
    ("deployment", KeywordKind::Deployment),
    ("diagnostic", KeywordKind::Diagnostic),
    ("dictionary", KeywordKind::Dictionary),
    ("do", KeywordKind::Do),
    ("drop", KeywordKind::Drop),
    ("else", KeywordKind::Else),
    ("enter", KeywordKind::Enter),
    ("entry", KeywordKind::Entry),
    ("enum", KeywordKind::Enum),
    ("event", KeywordKind::Event),
    ("every", KeywordKind::Every),
    ("exit", KeywordKind::Exit),
    ("expand", KeywordKind::Expand),
    ("external", KeywordKind::External),
    ("false", KeywordKind::False),
    ("fatal", KeywordKind::Fatal),
    ("format", KeywordKind::Format),
    ("get", KeywordKind::Get),
    ("group", KeywordKind::Group),
    ("guard", KeywordKind::Guard),
    ("guarded", KeywordKind::Guarded),
    ("health", KeywordKind::Health),
    ("high", KeywordKind::High),
    ("hook", KeywordKind::Hook),
    ("id", KeywordKind::Id),
    ("if", KeywordKind::If),
    ("implements", KeywordKind::Implements),
    ("import", KeywordKind::Import),
    ("include", KeywordKind::Include),
    ("initial", KeywordKind::Initial),
    ("input", KeywordKind::Input),
    ("instance", KeywordKind::Instance),
    ("interface", KeywordKind::Interface),
    ("internal", KeywordKind::Internal),
    ("locate", KeywordKind::Locate),
    ("low", KeywordKind::Low),
    ("machine", KeywordKind::Machine),
    ("match", KeywordKind::Match),
    ("module", KeywordKind::Module),
    ("omit", KeywordKind::Omit),
    ("on", KeywordKind::On),
    ("opcode", KeywordKind::Opcode),
    ("orange", KeywordKind::Orange),
    ("output", KeywordKind::Output),
    ("packet", KeywordKind::Packet),
    ("packets", KeywordKind::Packets),
    ("param", KeywordKind::Param),
    ("passive", KeywordKind::Passive),
    ("phase", KeywordKind::Phase),
    ("port", KeywordKind::Port),
    ("priority", KeywordKind::Priority),
    ("product", KeywordKind::Product),
    ("queue", KeywordKind::Queue),
    ("queued", KeywordKind::Queued),
    ("record", KeywordKind::Record),
    ("recv", KeywordKind::Recv),
    ("red", KeywordKind::Red),
    ("ref", KeywordKind::Ref),
    ("reg", KeywordKind::Reg),
    ("request", KeywordKind::Request),
    ("resp", KeywordKind::Resp),
    ("save", KeywordKind::Save),
    ("send", KeywordKind::Send),
    ("serial", KeywordKind::Serial),
    ("set", KeywordKind::Set),
    ("severity", KeywordKind::Severity),
    ("signal", KeywordKind::Signal),
    ("size", KeywordKind::Size),
    ("sizeof", KeywordKind::Sizeof),
    ("stack", KeywordKind::Stack),
    ("state", KeywordKind::State),
    ("string", KeywordKind::String_),
    ("struct", KeywordKind::Struct),
    ("sync", KeywordKind::Sync),
    ("system", KeywordKind::System),
    ("telemetry", KeywordKind::Telemetry),
    ("template", KeywordKind::Template),
    ("text", KeywordKind::Text),
    ("throttle", KeywordKind::Throttle),
    ("time", KeywordKind::Time),
    ("topology", KeywordKind::Topology),
    ("true", KeywordKind::True),
    ("type", KeywordKind::Type),
    ("unmatched", KeywordKind::Unmatched),
    ("update", KeywordKind::Update),
    ("warning", KeywordKind::Warning),
    ("with", KeywordKind::With),
    ("yellow", KeywordKind::Yellow),
];

impl KeywordKind {
    /// The source spelling of the keyword
    pub fn as_str(&self) -> &'static str {
        KEYWORDS
            .iter()
            .find(|(_, kind)| kind == self)
            .map(|(text, _)| *text)
            .expect("every keyword is in the keyword table")
    }

    /// The keyword spelled `text`, if there is one
    pub fn get(text: &str) -> Option<KeywordKind> {
        KEYWORDS
            .binary_search_by_key(&text, |(text, _)| text)
            .ok()
            .map(|i| KEYWORDS[i].1)
    }
}

impl std::fmt::Display for KeywordKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The kind of a token.
///
/// The lexer produces every character of the input as part of a token, so that
/// the lossless syntax tree of `fpp_lsp_parser` can be built from the tokens.
/// For that reason there are kinds for the text between the tokens of the
/// grammar: [`TokenKind::Whitespace`] (spaces and line continuations),
/// [`TokenKind::Comment`] and [`TokenKind::Unknown`] (an invalid character).
/// A multi-line string literal has a kind of its own,
/// [`TokenKind::LiteralMultilineString`].
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum TokenKind {
    EOF,
    Whitespace,
    Comment,
    Unknown,

    // Identifier (non keyword words)
    Identifier,

    // Annotations
    PostAnnotation,
    PreAnnotation,

    // Literals
    LiteralFloat,
    LiteralInt,
    LiteralString,
    /// A multi-line string literal whose first line has an initial space indent
    /// of `indent`
    LiteralMultilineString {
        indent: u32,
    },

    // Keywords
    Keyword(KeywordKind),

    // Symbols
    Colon,
    Comma,
    Dot,
    Eol,
    Equals,
    LeftCurly,
    LeftSquare,
    LeftParen,
    Minus,
    Plus,
    RightArrow,
    RightCurly,
    RightSquare,
    RightParen,
    Semi,
    Slash,
    Star,
    ShiftLeft,
    ShiftRight,
}

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TokenKind::EOF => "end of input",
            TokenKind::Whitespace => "<whitespace>",
            TokenKind::Unknown => "<unknown>",
            TokenKind::Comment => "comment",
            TokenKind::Identifier => "identifier",
            TokenKind::PostAnnotation => "post-annotation",
            TokenKind::PreAnnotation => "pre-annotation",
            TokenKind::LiteralFloat => "literal float",
            TokenKind::LiteralInt => "literal integer",
            TokenKind::LiteralString => "literal string",
            TokenKind::LiteralMultilineString { .. } => "literal multiline string",
            TokenKind::Keyword(keyword) => return std::fmt::Display::fmt(&keyword, f),
            TokenKind::Colon => ":",
            TokenKind::Comma => ",",
            TokenKind::Dot => ".",
            TokenKind::Eol => "end of line",
            TokenKind::Equals => "=",
            TokenKind::LeftCurly => "{",
            TokenKind::LeftSquare => "[",
            TokenKind::LeftParen => "(",
            TokenKind::Minus => "-",
            TokenKind::Plus => "+",
            TokenKind::RightArrow => "->",
            TokenKind::RightCurly => "}",
            TokenKind::RightSquare => "]",
            TokenKind::RightParen => ")",
            TokenKind::Semi => ";",
            TokenKind::Slash => "/",
            TokenKind::Star => "*",
            TokenKind::ShiftLeft => "<<",
            TokenKind::ShiftRight => ">>",
        };

        f.write_str(s)
    }
}

/// A token of the input
#[derive(Debug)]
pub struct Token {
    /// The kind of the token
    pub kind: TokenKind,
    /// The length of the token in bytes
    pub len: usize,
}
