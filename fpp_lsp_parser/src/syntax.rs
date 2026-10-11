#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(non_camel_case_types)]
#[repr(u16)]
pub enum SyntaxKind {
    #[doc(hidden)]
    TOMBSTONE,
    #[doc(hidden)]
    EOF,
    #[doc(hidden)]
    UNKNOWN,
    #[doc(hidden)]
    CURSOR,

    IDENT,
    POST_ANNOTATION,
    PRE_ANNOTATION,

    LITERAL_FLOAT,
    LITERAL_INT,
    LITERAL_STRING,

    // Keywords
    ACTION_KW,
    ACTIVE_KW,
    ACTIVITY_KW,
    ALWAYS_KW,
    ARRAY_KW,
    ASSERT_KW,
    ASYNC_KW,
    AT_KW,
    BASE_KW,
    BLOCK_KW,
    BOOL_KW,
    CHANGE_KW,
    COMMAND_KW,
    COMPONENT_KW,
    CONNECTIONS_KW,
    CONSTANT_KW,
    CONTAINER_KW,
    CPU_KW,
    DEFAULT_KW,
    DEPLOYMENT_KW,
    DIAGNOSTIC_KW,
    DICTIONARY_KW,
    DO_KW,
    DROP_KW,
    ELSE_KW,
    ENTER_KW,
    ENTRY_KW,
    ENUM_KW,
    EVENT_KW,
    EVERY_KW,
    EXIT_KW,
    EXPAND_KW,
    EXTERNAL_KW,
    F32_KW,
    F64_KW,
    FALSE_KW,
    FATAL_KW,
    FORMAT_KW,
    GET_KW,
    GROUP_KW,
    GUARD_KW,
    GUARDED_KW,
    HEALTH_KW,
    HIGH_KW,
    HOOK_KW,
    I16_KW,
    I32_KW,
    I64_KW,
    I8_KW,
    ID_KW,
    IF_KW,
    IMPLEMENTS_KW,
    IMPORT_KW,
    INCLUDE_KW,
    INITIAL_KW,
    INPUT_KW,
    INSTANCE_KW,
    INTERFACE_KW,
    INTERNAL_KW,
    CHOICE_KW,
    LOCATE_KW,
    LOW_KW,
    MACHINE_KW,
    MATCH_KW,
    MODULE_KW,
    OMIT_KW,
    ON_KW,
    OPCODE_KW,
    ORANGE_KW,
    OUTPUT_KW,
    PACKET_KW,
    PACKETS_KW,
    PARAM_KW,
    PASSIVE_KW,
    PHASE_KW,
    PORT_KW,
    PRIORITY_KW,
    PRODUCT_KW,
    QUEUE_KW,
    QUEUED_KW,
    RECORD_KW,
    RECV_KW,
    RED_KW,
    REF_KW,
    REG_KW,
    REQUEST_KW,
    RESP_KW,
    SAVE_KW,
    SEND_KW,
    SERIAL_KW,
    SET_KW,
    SEVERITY_KW,
    SIGNAL_KW,
    SIZE_KW,
    SIZEOF_KW,
    STACK_KW,
    STATE_KW,
    STRING_KW,
    STRUCT_KW,
    SYNC_KW,
    SYSTEM_KW,
    TELEMETRY_KW,
    TEMPLATE_KW,
    TEXT_KW,
    THROTTLE_KW,
    TIME_KW,
    TOPOLOGY_KW,
    TRUE_KW,
    TYPE_KW,
    U16_KW,
    U32_KW,
    U64_KW,
    U8_KW,
    UNMATCHED_KW,
    UPDATE_KW,
    WARNING_KW,
    WITH_KW,
    YELLOW_KW,

    // Symbols
    COLON,
    COMMA,
    DOT,
    EOL,
    EQUALS,

    LEFT_PAREN,
    LEFT_CURLY,
    LEFT_SQUARE,

    RIGHT_PAREN,
    RIGHT_CURLY,
    RIGHT_SQUARE,

    RIGHT_ARROW,
    MINUS,
    PLUS,
    SEMI,
    SLASH,
    STAR,
    SHIFT_LEFT,
    SHIFT_RIGHT,

    // Special
    WHITESPACE,
    COMMENT,
    ERROR,

    // Definitions
    DEF_ABSTRACT_TYPE,
    DEF_ALIAS_TYPE,
    DEF_ARRAY,
    DEF_COMPONENT,
    DEF_COMPONENT_INSTANCE,
    DEF_CONSTANT,
    DEF_ENUM,
    DEF_ENUM_CONSTANT,
    DEF_INTERFACE,
    DEF_MODULE,
    DEF_MODULE_TEMPLATE,
    DEF_PORT,
    DEF_STRUCT,
    DEF_SYSTEM,
    DEF_TOPOLOGY,

    DEF_ACTION,
    DEF_CHOICE,
    DEF_GUARD,
    DEF_SIGNAL,
    DEF_STATE,
    DEF_STATE_MACHINE,

    // Specifiers
    SPEC_COMMAND,
    SPEC_CONTAINER,
    SPEC_EVENT,
    SPEC_INCLUDE,
    SPEC_INIT,
    SPEC_INITIAL_TRANSITION,
    SPEC_INTERFACE_IMPORT,
    SPEC_LOC,
    SPEC_PORT_INSTANCE_GENERAL,
    SPEC_PORT_INSTANCE_SPECIAL,
    SPEC_PORT_INSTANCE_INTERNAL,
    SPEC_RECORD,
    SPEC_STATE_ENTRY,
    SPEC_STATE_EXIT,
    SPEC_STATE_MACHINE_INSTANCE,
    SPEC_STATE_TRANSITION,
    SPEC_TEMPLATE_EXPAND,
    SPEC_TELEMETRY,
    SPEC_PARAM,
    SPEC_INSTANCE,
    SPEC_TOP_PORT,
    SPEC_TLM_PACKET,
    SPEC_CONNECTION_GRAPH_PATTERN,
    SPEC_CONNECTION_GRAPH_DIRECT,

    // Helper nodes
    EXPR,
    EXPR_BINARY,
    EXPR_UNARY,
    EXPR_ARRAY_MEMBER_LIST,
    EXPR_ARRAY,
    EXPR_POSTFIX,
    EXPR_SUBSCRIPT,
    EXPR_MEMBER,
    EXPR_STRUCT,
    EXPR_STRUCT_MEMBER_LIST,
    EXPR_LITERAL,
    EXPR_IDENT,
    EXPR_SIZEOF,
    EXPR_STRUCT_MEMBER,
    BASE_ID,
    BINARY_OP,
    SUBSCRIPT,
    COMPONENT_INSTANCE_TYPE,
    COMPONENT_INSTANCE_FILE,
    COMPONENT_MEMBER_LIST,
    CPU,
    DEFAULT,
    DEFAULT_PRIORITY,
    DO_EXPR,
    DO_EXPR_MEMBER_LIST,
    ELSE_CLAUSE,
    ENUM_MEMBER_LIST,
    EVENT_THROTTLE,
    EVERY,
    FORMAL_PARAM,
    FORMAL_PARAM_LIST,
    FORMAT,
    GROUP,
    ID,
    INIT_SPEC_LIST,
    INTERFACE_MEMBER_LIST,
    LIMIT,
    LIMIT_SEQUENCE,
    MODULE_MEMBER_LIST,
    NAME,
    NAME_REF,
    OPCODE,
    PRIORITY,
    QUEUE_FULL,
    QUEUE_SIZE,
    EVENT_SEVERITY,
    SAVE_OPCODE,
    SET_OPCODE,
    INDEX_OR_SIZE,
    STACK_SIZE,
    STATE_MACHINE_MEMBER_LIST,
    STATE_MEMBER_LIST,
    STRUCT_MEMBER,
    STRUCT_MEMBER_LIST,
    TEMPLATE_ARG,
    TEMPLATE_ARG_LIST,
    TEMPLATE_PARAM,
    TEMPLATE_PARAM_LIST,
    THEN_CLAUSE,
    TRANSITION_EXPR,
    TYPE_NAME,
    QUAL_IDENT,
    IMPLEMENTS_CLAUSE,
    TOPOLOGY_MEMBER_LIST,
    PORT_INSTANCE_IDENTIFIER,
    TLM_PACKET_SET_MEMBER_LIST,
    TLM_PACKET_SET,
    TLM_PACKET_MEMBER_LIST,
    TLM_CHANNEL_IDENTIFIER,
    TLM_PACKET_OMIT,
    TLM_PACKET_OMIT_MEMBER_LIST,
    PATTERN_TARGET_MEMBER_LIST,
    CONNECTION_MEMBER_LIST,
    CONNECTION,
    CONNECTION_FROM,
    CONNECTION_TO,

    ROOT,

    #[doc(hidden)]
    __LAST,
}

use SyntaxKind::*;

impl From<u16> for SyntaxKind {
    #[inline]
    fn from(d: u16) -> SyntaxKind {
        assert!(d <= (__LAST as u16));
        unsafe { std::mem::transmute::<u16, SyntaxKind>(d) }
    }
}

/// Some boilerplate is needed, as rowan settled on using its own
/// `struct SyntaxKind(u16)` internally, instead of accepting the
/// user's `enum SyntaxKind` as a type parameter.
///
/// First, to easily pass the enum variants into rowan via `.into()`:
impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

impl SyntaxKind {
    #[inline]
    pub fn is_def(self) -> bool {
        matches!(
            self,
            DEF_ABSTRACT_TYPE
                | DEF_ALIAS_TYPE
                | DEF_ARRAY
                | DEF_COMPONENT
                | DEF_COMPONENT_INSTANCE
                | DEF_CONSTANT
                | DEF_ENUM
                | DEF_ENUM_CONSTANT
                | DEF_INTERFACE
                | DEF_MODULE
                | DEF_MODULE_TEMPLATE
                | DEF_PORT
                | DEF_STRUCT
                | DEF_ACTION
                | DEF_CHOICE
                | DEF_GUARD
                | DEF_SIGNAL
                | DEF_STATE
                | DEF_STATE_MACHINE
        )
    }

    #[inline]
    pub fn is_spec(self) -> bool {
        matches!(
            self,
            SPEC_COMMAND
                | SPEC_CONTAINER
                | SPEC_EVENT
                | SPEC_INCLUDE
                | SPEC_INIT
                | SPEC_INITIAL_TRANSITION
                | SPEC_INTERFACE_IMPORT
                | SPEC_LOC
                | SPEC_PORT_INSTANCE_GENERAL
                | SPEC_PORT_INSTANCE_SPECIAL
                | SPEC_PORT_INSTANCE_INTERNAL
                | SPEC_RECORD
                | SPEC_STATE_ENTRY
                | SPEC_STATE_EXIT
                | SPEC_STATE_MACHINE_INSTANCE
                | SPEC_STATE_TRANSITION
                | SPEC_TEMPLATE_EXPAND
                | SPEC_TELEMETRY
                | SPEC_PARAM
        )
    }

    #[inline]
    pub fn is_modifier_keyword(self) -> bool {
        matches!(
            self,
            ACTIVE_KW | ASYNC_KW | SYNC_KW | PASSIVE_KW | QUEUED_KW | GUARDED_KW
        )
    }

    #[inline]
    pub fn is_type_primitive_keyword(self) -> bool {
        matches!(
            self,
            U8_KW
                | I8_KW
                | U16_KW
                | I16_KW
                | U32_KW
                | I32_KW
                | U64_KW
                | I64_KW
                | F32_KW
                | F64_KW
                | STRING_KW
                | BOOL_KW
        )
    }

    #[inline]
    pub fn is_keyword(self) -> bool {
        matches!(
            self,
            ACTION_KW
                | ACTIVE_KW
                | ACTIVITY_KW
                | ALWAYS_KW
                | ARRAY_KW
                | ASSERT_KW
                | ASYNC_KW
                | AT_KW
                | BASE_KW
                | BLOCK_KW
                | BOOL_KW
                | CHANGE_KW
                | COMMAND_KW
                | COMPONENT_KW
                | CONNECTIONS_KW
                | CONSTANT_KW
                | CONTAINER_KW
                | CPU_KW
                | DEFAULT_KW
                | DEPLOYMENT_KW
                | DIAGNOSTIC_KW
                | DICTIONARY_KW
                | DO_KW
                | DROP_KW
                | ELSE_KW
                | ENTER_KW
                | ENTRY_KW
                | ENUM_KW
                | EVENT_KW
                | EVERY_KW
                | EXIT_KW
                | EXPAND_KW
                | EXTERNAL_KW
                | F32_KW
                | F64_KW
                | FALSE_KW
                | FATAL_KW
                | FORMAT_KW
                | GET_KW
                | GROUP_KW
                | GUARD_KW
                | GUARDED_KW
                | HEALTH_KW
                | HIGH_KW
                | HOOK_KW
                | I16_KW
                | I32_KW
                | I64_KW
                | I8_KW
                | ID_KW
                | IF_KW
                | IMPLEMENTS_KW
                | IMPORT_KW
                | INCLUDE_KW
                | INITIAL_KW
                | INPUT_KW
                | INSTANCE_KW
                | INTERFACE_KW
                | INTERNAL_KW
                | CHOICE_KW
                | LOCATE_KW
                | LOW_KW
                | MACHINE_KW
                | MATCH_KW
                | MODULE_KW
                | OMIT_KW
                | ON_KW
                | OPCODE_KW
                | ORANGE_KW
                | OUTPUT_KW
                | PACKET_KW
                | PACKETS_KW
                | PARAM_KW
                | PASSIVE_KW
                | PHASE_KW
                | PORT_KW
                | PRIORITY_KW
                | PRODUCT_KW
                | QUEUE_KW
                | QUEUED_KW
                | RECORD_KW
                | RECV_KW
                | RED_KW
                | REF_KW
                | REG_KW
                | REQUEST_KW
                | RESP_KW
                | SAVE_KW
                | SEND_KW
                | SERIAL_KW
                | SET_KW
                | SEVERITY_KW
                | SIGNAL_KW
                | SIZE_KW
                | SIZEOF_KW
                | STACK_KW
                | STATE_KW
                | STRING_KW
                | STRUCT_KW
                | SYNC_KW
                | SYSTEM_KW
                | TELEMETRY_KW
                | TEMPLATE_KW
                | TEXT_KW
                | THROTTLE_KW
                | TIME_KW
                | TOPOLOGY_KW
                | TRUE_KW
                | TYPE_KW
                | U16_KW
                | U32_KW
                | U64_KW
                | U8_KW
                | UNMATCHED_KW
                | UPDATE_KW
                | WARNING_KW
                | WITH_KW
                | YELLOW_KW
        )
    }

    /// The name of this kind in a diagnostic: the source text of a symbol or
    /// a keyword, or the name of the construct (Scala's `accept` names).
    pub fn display_name(self) -> String {
        let symbol = match self {
            IDENT | QUAL_IDENT | NAME | NAME_REF => "identifier",
            EXPR => "expression",
            TYPE_NAME => "type name",
            LITERAL_FLOAT => "floating-point literal",
            LITERAL_INT => "integer literal",
            LITERAL_STRING => "string literal",
            PRE_ANNOTATION => "pre annotation",
            POST_ANNOTATION => "post annotation",
            EOF => "end of input",
            EOL => "end of line",
            COLON => ":",
            COMMA => ",",
            DOT => ".",
            EQUALS => "=",
            LEFT_PAREN => "(",
            LEFT_CURLY => "{",
            LEFT_SQUARE => "[",
            RIGHT_PAREN => ")",
            RIGHT_CURLY => "}",
            RIGHT_SQUARE => "]",
            RIGHT_ARROW => "->",
            MINUS => "-",
            PLUS => "+",
            SEMI => ";",
            SLASH => "/",
            STAR => "*",
            SHIFT_LEFT => "<<",
            SHIFT_RIGHT => ">>",
            _ => "",
        };
        if !symbol.is_empty() {
            return symbol.to_string();
        }
        let name = format!("{:?}", self);
        match name.strip_suffix("_KW") {
            // The integer and floating-point type keywords are upper case
            Some(kw)
                if self.is_type_primitive_keyword() && !matches!(self, BOOL_KW | STRING_KW) =>
            {
                kw.to_string()
            }
            Some(kw) => kw.to_ascii_lowercase(),
            None => name.to_ascii_lowercase().replace('_', " "),
        }
    }

    /// Check if this syntax kind is a member list that should be indented
    /// Used by the formatter to determine indentation and member separation
    #[inline]
    pub fn is_member_list(self) -> bool {
        matches!(
            self,
            MODULE_MEMBER_LIST
                | DO_EXPR_MEMBER_LIST
                | COMPONENT_MEMBER_LIST
                | INTERFACE_MEMBER_LIST
                | STRUCT_MEMBER_LIST
                | ENUM_MEMBER_LIST
                | STATE_MACHINE_MEMBER_LIST
                | STATE_MEMBER_LIST
                | TOPOLOGY_MEMBER_LIST
                | TLM_PACKET_SET_MEMBER_LIST
                | TLM_PACKET_MEMBER_LIST
                | TLM_PACKET_OMIT_MEMBER_LIST
                | CONNECTION_MEMBER_LIST
                | LIMIT_SEQUENCE
                | INIT_SPEC_LIST
                | FORMAL_PARAM_LIST
                | TEMPLATE_PARAM_LIST
                | TEMPLATE_ARG_LIST
                | EXPR_STRUCT_MEMBER_LIST
                | EXPR_ARRAY_MEMBER_LIST
                | PATTERN_TARGET_MEMBER_LIST
        )
    }
}
