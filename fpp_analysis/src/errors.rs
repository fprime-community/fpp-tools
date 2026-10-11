use crate::semantics::{TypeConversionError, UseDefMatching};
use fpp_core::{Diagnostic, Level, Span, Spanned};
use std::cell::RefCell;

/// A semantic error
#[derive(Debug)]
pub enum SemanticError {
    /// An error with notes
    AnnotatedError {
        error: Box<SemanticError>,
        notes: Vec<String>,
    },
    /// Empty array
    EmptyArray {
        loc: Span,
    },
    /// Channel not in dictionary
    ChannelNotInDictionary {
        loc: Span,
        channel_name: String,
        topology_name: String,
    },
    /// Division by zero
    DivisionByZero {
        loc: Span,
    },
    /// Duplicate connection at matched port
    DuplicateConnectionAtMatchedPort {
        loc: Span,
        port: String,
        port_num: i128,
        prev_loc: Span,
        matching_loc: Span,
    },
    /// Duplicate name in dictionary
    DuplicateDictionaryName {
        kind: String,
        name: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate enum value
    DuplicateEnumValue {
        value: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate ID value
    DuplicateIdValue {
        value: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate init specifier
    DuplicateInitSpecifier {
        phase: i128,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate instance
    DuplicateInstance {
        name: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate telemetry channel limit
    DuplicateLimit {
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate matched connection
    DuplicateMatchedConnection {
        loc: Span,
        prev_loc: Span,
        matching_loc: Span,
    },
    /// Duplicate opcode value
    DuplicateOpcodeValue {
        value: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate output port
    DuplicateOutputConnection {
        loc: Span,
        port_num: i128,
        prev_loc: Span,
    },
    /// Duplicate parameter
    DuplicateParameter {
        name: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate pattern
    DuplicatePattern {
        kind: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate port instance
    DuplicatePortInstance {
        name: String,
        loc: Span,
        import_locs: Vec<Span>,
        prev_loc: Span,
        prev_import_locs: Vec<Span>,
    },
    /// Duplicate state machine instance
    DuplicateStateMachineInstance {
        name: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate struct member
    DuplicateStructMember {
        name: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate system definition
    DuplicateSystemDefinition {
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate telemetry packet set
    DuplicateTlmPacketSet {
        name: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate interface
    DuplicateInterface {
        name: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Implicit duplicate connection at matched port
    ImplicitDuplicateConnectionAtMatchedPort {
        loc: Span,
        port: String,
        port_num: i128,
        implying_loc: Span,
        matching_loc: Span,
        prev_loc: Span,
    },
    /// Inconsistent dictionary specifiers in location specifiers
    InconsistentDictionarySpecifier {
        /// Location of the location specifier
        loc: Span,
        /// Location of the previous specifier
        prev_loc: Span,
    },
    /// Inconsistent location specifiers
    InconsistentLocationPath {
        /// Location of the file string literal of this specifier
        loc: Span,
        /// The path named by this specifier
        path: String,
        /// Location of the file string literal of the previous specifier for the
        /// same symbol
        prev_loc: Span,
        /// The path named by the previous specifier
        prev_path: String,
    },
    /// Incorrect dictionary specifier in location specifier
    IncorrectDictionarySpecifier {
        /// Location of the location specifier
        loc: Span,
        /// Location of the actual definition
        def_loc: Span,
    },
    /// Incorrect location specifiers
    IncorrectLocationPath {
        /// Location of the file string literal in the location specifier
        loc: Span,
        /// The path named by the specifier
        specified_path: String,
        /// Location of the actual definition (in the translation unit)
        actual_loc: Span,
    },
    /// No member in anonymous struct type
    InvalidAnonStructMember {
        member_name: String,
        loc: Span,
        /// The definition of the symbol whose value has no such member, if
        /// the selection is from a symbol
        def_loc: Option<Span>,
    },
    /// Invalid array size
    InvalidArraySize {
        loc: Span,
        size: i128,
    },
    /// Invalid command
    InvalidCommand {
        loc: Span,
        msg: String,
    },
    /// Invalid interface instance
    InvalidInterfaceInstance {
        loc: Span,
        instance_name: String,
        top_name: String,
    },
    /// Invalid connection
    InvalidConnection {
        loc: Span,
        msg: String,
        from_loc: Span,
        to_loc: Span,
        from_port_def_loc: Option<Span>,
        to_port_def_loc: Option<Span>,
    },
    /// Invalid data products
    InvalidDataProducts {
        loc: Span,
        msg: String,
    },
    /// Invalid component instance definition
    InvalidDefComponentInstance {
        name: String,
        loc: Span,
        msg: String,
    },
    /// Invalid enum constants
    InvalidEnumConstants {
        loc: Span,
    },
    /// Invalid event
    InvalidEvent {
        loc: Span,
        msg: String,
    },
    /// Invalid format string
    InvalidFormatString {
        loc: Span,
        msg: String,
        /// For a string that does not parse and whose error cannot be pointed
        /// at in the source: the line of the string holding the error, with a
        /// caret under the error position
        context: Option<String>,
    },
    /// Invalid integer value
    InvalidIntValue {
        loc: Span,
        v: Option<i128>,
        msg: String,
    },
    /// Invalid internal port
    InvalidInternalPort {
        loc: Span,
        msg: String,
    },
    /// Invalid connection pattern
    InvalidPattern {
        loc: Span,
        msg: String,
    },
    /// Invalid port instance
    InvalidPortInstance {
        loc: Span,
        msg: String,
        def_loc: Span,
    },
    /// Invalid port instance identifier
    InvalidPortInstanceId {
        loc: Span,
        port_name: String,
        symbol_kind: String,
        interface_name: String,
    },
    /// Invalid port kind
    InvalidPortKind {
        loc: Span,
        msg: String,
        spec_loc: Span,
    },
    /// Invalid port matching
    InvalidPortMatching {
        loc: Span,
        msg: String,
    },
    /// Invalid port number
    InvalidPortNumber {
        loc: Span,
        port_number: i128,
        port: String,
        size: i128,
        spec_loc: Span,
    },
    /// Invalid priority specifier
    InvalidPriority {
        loc: Span,
    },
    /// Invalid queue full specifier
    InvalidQueueFull {
        loc: Span,
    },
    /// Invalid shift amount
    InvalidShiftAmount {
        loc: Span,
    },
    /// Invalid special port
    InvalidSpecialPort {
        loc: Span,
        msg: String,
    },
    /// Invalid string size
    InvalidStringSize {
        loc: Span,
        size: i128,
    },
    /// Invalid qualifier
    InvalidQualifier {
        name: String,
        loc: Span,
        msg: String,
        def_loc: Span,
    },
    /// No member in struct type
    InvalidStructMember {
        member_name: String,
        loc: Span,
        struct_type_name: String,
        def_loc: Span,
    },
    /// No member in a value whose type has no members
    InvalidTypeForMemberSelection {
        member_name: String,
        loc: Span,
        type_name: String,
    },
    /// Invalid symbol
    InvalidSymbol {
        name: String,
        loc: Span,
        msg: String,
        def_loc: Span,
    },
    /// Invalid telemetry channel identifier name
    InvalidTlmChannelName {
        loc: Span,
        channel_name: String,
        component_name: String,
    },
    /// Invalid telemetry packet set
    InvalidTlmPacketSet {
        loc: Span,
        name: String,
        msg: String,
    },
    /// Telemetry channels of a packet set that are neither used nor marked as
    /// omitted
    TlmPacketSetChannelsNotCovered {
        loc: Span,
        name: String,
        /// The qualified names of the uncovered channels, in dictionary order
        channels: Vec<String>,
    },
    /// Telemetry channels of a packet set that are both used and marked as
    /// omitted
    TlmPacketSetChannelsUsedAndOmitted {
        loc: Span,
        name: String,
        /// For each channel, its qualified name, a location where it is used,
        /// and a location where it is marked as omitted, in dictionary order
        channels: Vec<(String, Span, Span)>,
    },
    /// Invalid type
    InvalidType {
        loc: Span,
        msg: String,
        /// The chain of locations explaining why the type is invalid
        notes: Vec<(Span, String)>,
    },
    /// Mismatched port numbers
    MismatchedPortNumbers {
        p1_loc: Span,
        p1_number: i128,
        p2_loc: Span,
        p2_number: i128,
        matching_loc: Span,
    },
    /// Wrong number of template arguments
    WrongNumberOfTemplateArguments {
        expand_loc: Span,
        def_loc: Span,
        expand_length: usize,
        def_length: usize,
    },
    /// Template argument of the wrong kind
    InvalidTemplateArg {
        param_name: String,
        arg_loc: Span,
        param_loc: Span,
        msg: String,
    },
    /// Template parameter conflicts with a definition in the template body
    TemplateParameterConflict {
        param_name: String,
        param_loc: Span,
        def_loc: Span,
    },
    /// Nested template definition
    NestedTemplateDefinition {
        nested_loc: Span,
        parent_loc: Span,
    },
    /// Template expansion specifier nested inside a template
    TemplateExpansionInTemplate {
        expand_loc: Span,
        template_loc: Span,
    },
    /// Missing async input
    MissingAsync {
        kind: String,
        loc: Span,
    },
    /// Missing connection
    MissingConnection {
        loc: Span,
        matching_loc: Span,
    },
    /// Missing port
    MissingPort {
        loc: Span,
        spec_msg: String,
        port_msg: String,
    },
    MissingPortMatching {
        loc: Span,
    },
    /// Matched port numbering could not find a valid port number
    NoPortAvailableForMatchedNumbering {
        loc1: Span,
        loc2: Span,
        matching_loc: Span,
    },
    /// Overlapping ID ranges
    OverlappingIdRanges {
        base_id1: i128,
        name1: String,
        loc1: Span,
        base_id2: i128,
        max_id2: i128,
        name2: String,
        loc2: Span,
    },
    /// Error while importing an interface
    InterfaceImport {
        import_loc: Span,
        err: Box<SemanticError>,
    },
    /// A port is missing from a port interface
    PortInterfaceMissingPort {
        loc: Span,
    },
    /// A port does not match an expected signature
    PortInterfaceInvalidPort {
        loc: Span,
        def_loc: Span,
    },
    /// Error while checking whether an instance implements an interface
    InterfaceImplements {
        loc: Span,
        err: Box<SemanticError>,
    },
    /// Passive async input
    PassiveAsync {
        loc: Span,
        import_locs: Vec<Span>,
    },
    PassiveStateMachine {
        loc: Span,
    },
    /// Redefined symbol
    RedefinedSymbol {
        /// Name of the symbol being redefined
        name: String,
        /// Location of the duplicate symbol
        loc: Span,
        /// Location of the previous symbol that is clashing
        prev_loc: Span,
    },

    // State machine semantic errors
    /// Call site type mismatch
    CallSiteTypeMismatch {
        loc: Span,
        te_kind: String,
        te_ty: String,
        site_kind: String,
        site_ty: String,
    },
    /// Duplicate signal
    DuplicateSignal {
        sig_name: String,
        state_name: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Invalid initial transition specifier
    InvalidInitialTransition {
        loc: Span,
        msg: String,
        /// Locations along the transition path (rendered as notes)
        path: Vec<Span>,
        /// The initial transitions of a state or state machine that has more
        /// than one (rendered as notes)
        duplicate: Vec<Span>,
    },
    /// Choice cycle
    ChoiceCycle {
        loc: Span,
        /// The transitions of the cycle, each with its location (rendered as
        /// notes)
        path: Vec<(Span, String)>,
    },
    /// Choice type mismatch
    ChoiceTypeMismatch {
        loc: Span,
        to_loc1: Span,
        to1: String,
        to_loc2: Span,
        to2: String,
    },
    /// Unreachable node in the transition graph
    UnreachableNode {
        name: String,
        loc: Span,
    },
    // End of state machine semantic errors
    /// Too many output ports
    TooManyOutputPorts {
        loc: Span,
        num_ports: i128,
        array_size: i128,
        instance_loc: Span,
    },
    /// Type mismatch
    TypeMismatch {
        loc: Span,
        msg: String,
        /// Why the conversion failed, when the mismatch is a failed type
        /// conversion. Rendered as notes after the message.
        reason: Option<Box<TypeConversionError>>,
    },
    /// Undefined symbol
    UndefinedSymbol {
        name: String,
        symbol_kind: String,
        loc: Span,
    },
    /// Use-def cycle
    UseDefCycle {
        loc: Span,
        /// The use-def matchings on the path that closes the cycle, in the
        /// order they were followed
        use_def_matching_list: Vec<UseDefMatching>,
    },
}

/// The result of a compilation step
pub type SemanticResult<T = ()> = Result<T, SemanticError>;

/// Wraps error as an AnnotatedError with notes
pub fn annotate_result<T>(r: SemanticResult<T>, notes: &[String]) -> SemanticResult<T> {
    match r {
        Err(error) if !notes.is_empty() => Err(SemanticError::AnnotatedError {
            error: Box::new(error),
            notes: notes.to_vec(),
        }),
        r => r,
    }
}

thread_local! {
    /// The notes of the [`annotate_emitted_errors`] calls in progress on this
    /// thread, outermost first
    static EMISSION_NOTES: RefCell<Vec<Vec<String>>> = const { RefCell::new(Vec::new()) };
}

/// Runs `f`, annotating every error that `f` emits with `notes`, as
/// [`annotate_result`] annotates an error result.
///
/// A pass emits an error where it finds it and goes on, rather than returning
/// it to the step that started the work. So to annotate the errors of a step,
/// the notes are attached when an error is emitted during the step. Calls
/// nest: an error emitted inside several of them carries the notes of the
/// innermost call first.
pub fn annotate_emitted_errors<T>(notes: Vec<String>, f: impl FnOnce() -> T) -> T {
    /// Removes the notes of a call when the call ends, even by unwinding
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            EMISSION_NOTES.with_borrow_mut(|stack| stack.pop());
        }
    }

    if notes.is_empty() {
        return f();
    }
    EMISSION_NOTES.with_borrow_mut(|stack| stack.push(notes));
    let _guard = Guard;
    f()
}

impl SemanticError {
    /// Print the error
    pub fn emit(self) {
        let error = EMISSION_NOTES.with_borrow(|stack| {
            stack
                .iter()
                .rev()
                .fold(self, |error, notes| SemanticError::AnnotatedError {
                    error: Box::new(error),
                    notes: notes.clone(),
                })
        });
        Into::<Diagnostic>::into(error).emit();
    }
}

/// Renders `n` as a count of telemetry channels, e.g. `1 telemetry channel`
fn count_telemetry_channels(n: usize) -> String {
    format!("{} telemetry channel{}", n, if n == 1 { "" } else { "s" })
}

/// The notes that several errors share
trait SharedNotes {
    /// Note the location of a port matching
    fn matching_loc_note(self, matching_loc: Span) -> Self;

    /// Note the location of a previous occurrence
    fn prev_loc_note(self, prev_loc: Span) -> Self;
}

impl SharedNotes for Diagnostic {
    fn matching_loc_note(self, matching_loc: Span) -> Self {
        self.span_note(matching_loc, "port matching is specified here")
    }

    fn prev_loc_note(self, prev_loc: Span) -> Self {
        self.span_note(prev_loc, "previous occurrence is here")
    }
}

impl From<SemanticError> for Diagnostic {
    fn from(val: SemanticError) -> Self {
        match val {
            SemanticError::AnnotatedError { error, notes } => notes
                .into_iter()
                .fold(Diagnostic::from(*error), |out, note| out.note(note)),
            SemanticError::EmptyArray { loc } => {
                Diagnostic::new(loc, Level::Error, "array expression may not be empty")
            }
            SemanticError::ChannelNotInDictionary {
                loc,
                channel_name,
                topology_name,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!(
                    "channel {} is not in the dictionary for topology {}",
                    channel_name, topology_name
                ),
            ),
            SemanticError::DivisionByZero { loc } => {
                Diagnostic::new(loc, Level::Error, "division by zero")
            }
            SemanticError::DuplicateConnectionAtMatchedPort {
                loc,
                port,
                port_num,
                prev_loc,
                matching_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate connection at matched port {}[{}]", port, port_num),
            )
            .prev_loc_note(prev_loc)
            .matching_loc_note(matching_loc),
            SemanticError::DuplicateDictionaryName {
                kind,
                name,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate {} name {}", kind, name),
            )
            .prev_loc_note(prev_loc),
            SemanticError::DuplicateEnumValue {
                value,
                loc,
                prev_loc,
            } => Diagnostic::new(loc, Level::Error, format!("duplicate enum value {}", value))
                .prev_loc_note(prev_loc),
            SemanticError::DuplicateIdValue {
                value,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate identifier value {}", value),
            )
            .prev_loc_note(prev_loc),
            SemanticError::DuplicateInitSpecifier {
                phase,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate initialization for phase {}", phase),
            )
            .prev_loc_note(prev_loc),
            SemanticError::DuplicateInstance {
                name,
                loc,
                prev_loc,
            } => Diagnostic::new(loc, Level::Error, format!("duplicate instance {}", name))
                .span_note(prev_loc, "previous instance is here"),
            SemanticError::DuplicateLimit { loc, prev_loc } => {
                Diagnostic::new(loc, Level::Error, "duplicate limit")
                    .prev_loc_note(prev_loc)
            }
            SemanticError::DuplicateMatchedConnection {
                loc,
                prev_loc,
                matching_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                "duplicate connection between a matched port array and a single instance",
            )
            .prev_loc_note(prev_loc)
            .matching_loc_note(matching_loc)
            .note("each port in a matched port array must be connected to a separate instance"),
            SemanticError::DuplicateOpcodeValue {
                value,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate opcode value {}", value),
            )
            .prev_loc_note(prev_loc),
            SemanticError::DuplicateOutputConnection {
                loc,
                port_num,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate connection at output port {}", port_num),
            )
            .prev_loc_note(prev_loc),
            SemanticError::DuplicateParameter {
                name,
                loc,
                prev_loc,
            } => Diagnostic::new(loc, Level::Error, format!("duplicate parameter {}", name))
                .span_note(prev_loc, "previous parameter is here"),
            SemanticError::DuplicatePattern {
                kind,
                loc,
                prev_loc,
            } => Diagnostic::new(loc, Level::Error, format!("duplicate {} pattern", kind))
                .prev_loc_note(prev_loc),
            SemanticError::DuplicatePortInstance {
                name,
                loc,
                import_locs,
                prev_loc,
                prev_import_locs,
            } => {
                let diag = Diagnostic::new(
                    loc,
                    Level::Error,
                    format!("duplicate port instance {}", name),
                );
                let diag = import_locs
                    .iter()
                    .fold(diag, |diag, l| diag.span_note(*l, "port imported from here"));
                let diag = diag.span_note(prev_loc, "previous instance is here");
                prev_import_locs.iter().rev().fold(diag, |diag, l| {
                    diag.span_note(*l, "previous instance imported from here")
                })
            }
            SemanticError::DuplicateStateMachineInstance {
                name,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate state machine instance {}", name),
            )
            .prev_loc_note(prev_loc),
            SemanticError::DuplicateStructMember {
                name,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate struct member `{}`", name),
            )
            .span_note(prev_loc, "previously defined here"),
            SemanticError::DuplicateSystemDefinition { loc, prev_loc } => {
                Diagnostic::new(loc, Level::Error, "duplicate system definition")
                    .span_note(prev_loc, "previous definition is here")
                    .note("each model may have at most one system definition")
            }
            SemanticError::DuplicateTlmPacketSet {
                name,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate telemetry packet set {}", name),
            )
            .span_note(prev_loc, "previous set is here"),
            SemanticError::DuplicateInterface {
                name,
                loc,
                prev_loc,
            } => Diagnostic::new(loc, Level::Error, format!("duplicate interface {}", name))
                .prev_loc_note(prev_loc),
            SemanticError::ImplicitDuplicateConnectionAtMatchedPort {
                loc,
                port,
                port_num,
                implying_loc,
                matching_loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("implicit duplicate connection at matched port {}[{}]", port, port_num),
            )
            .span_note(implying_loc, "connection is implied here")
            .span_note(matching_loc, "because of matching specified here")
            .span_note(prev_loc, "conflicting connection is here"),
            SemanticError::InconsistentDictionarySpecifier { loc, prev_loc } => {
                Diagnostic::new(loc, Level::Error, "inconsistent location specifier")
                    .prev_loc_note(prev_loc)
                    .note("one specifies dictionary and one does not")
            }
            SemanticError::InconsistentLocationPath {
                loc,
                path,
                prev_loc,
                prev_path,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("inconsistent location path {}", path),
            )
            .span_note(prev_loc, format!("previous path {} is here", prev_path)),
            SemanticError::IncorrectDictionarySpecifier { loc, def_loc } => {
                Diagnostic::new(loc, Level::Error, "incorrect location specifier")
                    .span_note(def_loc, "actual definition is here")
                    .note("one specifies dictionary and one does not")
            }
            SemanticError::IncorrectLocationPath {
                loc,
                specified_path,
                actual_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("incorrect location path {}", specified_path),
            )
            .span_note(actual_loc, "actual location is here"),
            SemanticError::InvalidAnonStructMember {
                member_name,
                loc,
                def_loc,
            } => {
                let diag = Diagnostic::new(
                    loc,
                    Level::Error,
                    format!("no member {} in anonymous struct value", member_name),
                );
                match def_loc {
                    Some(def_loc) => diag.span_note(def_loc, "symbol is defined here"),
                    None => diag,
                }
            }
            SemanticError::InvalidArraySize { loc, size } => Diagnostic::new(
                loc,
                Level::Error,
                format!("invalid array size {}", size),
            )
            .note("size must be greater than zero"),
            SemanticError::InvalidCommand { loc, msg } => Diagnostic::new(loc, Level::Error, msg),
            SemanticError::InvalidInterfaceInstance {
                loc,
                instance_name,
                top_name,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!(
                    "instance {} is not a member of topology {}",
                    instance_name, top_name
                ),
            ),
            SemanticError::InvalidConnection {
                loc,
                msg,
                from_loc,
                to_loc,
                from_port_def_loc,
                to_port_def_loc,
            } => {
                let mut d = Diagnostic::new(loc, Level::Error, msg)
                    .span_note(from_loc, "from port is specified here")
                    .span_note(to_loc, "to port is specified here");
                if let Some(l) = from_port_def_loc {
                    d = d.span_note(l, "from port type is defined here");
                }
                if let Some(l) = to_port_def_loc {
                    d = d.span_note(l, "to port type is defined here");
                }
                d
            }
            SemanticError::InvalidDataProducts { loc, msg } => {
                Diagnostic::new(loc, Level::Error, msg)
            }
            SemanticError::InvalidDefComponentInstance { name, loc, msg } => Diagnostic::new(
                loc,
                Level::Error,
                format!("invalid component instance definition {}: {}", name, msg),
            ),
            SemanticError::InvalidEnumConstants { loc } => Diagnostic::new(
                loc,
                Level::Error,
                "enum constants must be all explicit or all implied",
            ),
            SemanticError::InvalidEvent { loc, msg } => Diagnostic::new(loc, Level::Error, msg),
            SemanticError::InvalidFormatString { loc, msg, context } => {
                let diag =
                    Diagnostic::new(loc, Level::Error, format!("invalid format string: {}", msg));
                match context {
                    Some(context) => diag.note(context),
                    None => diag,
                }
            }
            SemanticError::InvalidIntValue { loc, v, msg } => match v {
                None => Diagnostic::new(loc, Level::Error, msg),
                Some(v) => {
                    Diagnostic::new(loc, Level::Error, format!("invalid integer value {}", v))
                        .note(msg)
                }
            },
            SemanticError::InvalidInternalPort { loc, msg } => {
                Diagnostic::new(loc, Level::Error, msg)
            }
            SemanticError::InvalidPattern { loc, msg } => Diagnostic::new(loc, Level::Error, msg),
            SemanticError::InvalidPortInstance { loc, msg, def_loc } => {
                Diagnostic::new(loc, Level::Error, msg).span_note(def_loc, "port definition is here")
            }
            SemanticError::InvalidPortInstanceId {
                loc,
                port_name,
                symbol_kind,
                interface_name,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!(
                    "{} is not a port instance of {} {}",
                    port_name, symbol_kind, interface_name
                ),
            ),
            SemanticError::InvalidPortKind { loc, msg, spec_loc } => {
                Diagnostic::new(loc, Level::Error, msg)
                    .span_note(spec_loc, "port instance is specified here")
            }
            SemanticError::InvalidPortMatching { loc, msg } => {
                Diagnostic::new(loc, Level::Error, msg)
            }
            SemanticError::InvalidPortNumber {
                loc,
                port_number,
                port,
                size,
                spec_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!(
                    "invalid port number {} for port {} (max is {})",
                    port_number,
                    port,
                    size - 1
                ),
            )
            .span_note(spec_loc, "port instance is specified here"),
            SemanticError::InvalidPriority { loc } => Diagnostic::new(
                loc,
                Level::Error,
                "only async input may have a priority",
            ),
            SemanticError::InvalidQueueFull { loc } => Diagnostic::new(
                loc,
                Level::Error,
                "only async input may have queue full behavior",
            ),
            SemanticError::InvalidShiftAmount { loc } => {
                Diagnostic::new(loc, Level::Error, "invalid shift amount").note(
                    "shift amount must be a non-negative value that must be in the range [0,255]",
                )
            }
            SemanticError::InvalidSpecialPort { loc, msg } => {
                Diagnostic::new(loc, Level::Error, msg)
            }
            SemanticError::InvalidStringSize { loc, size } => Diagnostic::new(
                loc,
                Level::Error,
                format!("invalid string size {}", size),
            ),
            SemanticError::InvalidQualifier {
                name,
                loc,
                msg,
                def_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("invalid qualifier {}: {}", name, msg),
            )
            .span_note(def_loc, "symbol is defined here"),
            SemanticError::InvalidStructMember {
                member_name,
                loc,
                struct_type_name,
                def_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!(
                    "no member {} in struct type {}",
                    member_name, struct_type_name
                ),
            )
            .span_note(def_loc, "symbol is defined here"),
            SemanticError::InvalidTypeForMemberSelection {
                member_name,
                loc,
                type_name,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("no member {} in value of type {}", member_name, type_name),
            ),
            SemanticError::InvalidSymbol {
                name,
                loc,
                msg,
                def_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("invalid use of symbol {}: {}", name, msg),
            )
            .span_note(def_loc, "symbol is defined here"),
            SemanticError::InvalidTlmChannelName {
                loc,
                channel_name,
                component_name,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!(
                    "{} is not a telemetry channel of component {}",
                    channel_name, component_name
                ),
            ),
            SemanticError::InvalidTlmPacketSet { loc, name, msg } => Diagnostic::new(
                loc,
                Level::Error,
                format!("invalid telemetry packet set {}", name),
            )
            .note(msg),
            SemanticError::TlmPacketSetChannelsNotCovered {
                loc,
                name,
                channels,
            } => {
                let diag = Diagnostic::new(
                    loc,
                    Level::Error,
                    format!("invalid telemetry packet set {}", name),
                )
                .note(format!(
                    "{} {} neither used nor marked as omitted",
                    count_telemetry_channels(channels.len()),
                    if channels.len() == 1 { "is" } else { "are" },
                ));
                channels.into_iter().fold(diag, Diagnostic::note)
            }
            SemanticError::TlmPacketSetChannelsUsedAndOmitted {
                loc,
                name,
                channels,
            } => {
                let diag = Diagnostic::new(
                    loc,
                    Level::Error,
                    format!("invalid telemetry packet set {}", name),
                )
                .note(format!(
                    "{} {} both used and marked as omitted",
                    count_telemetry_channels(channels.len()),
                    if channels.len() == 1 { "is" } else { "are" },
                ));
                channels
                    .into_iter()
                    .fold(diag, |diag, (channel, used_loc, omitted_loc)| {
                        diag.span_note(used_loc, format!("{} is used here", channel))
                            .span_note(omitted_loc, format!("{} is marked omitted here", channel))
                    })
            }
            SemanticError::InvalidType { loc, msg, notes } => notes
                .into_iter()
                .fold(Diagnostic::new(loc, Level::Error, msg), |diag, (loc, note)| {
                    diag.span_note(loc, note)
                }),
            SemanticError::MismatchedPortNumbers {
                p1_loc,
                p1_number,
                p2_loc,
                p2_number,
                matching_loc,
            } => Diagnostic::new(
                p1_loc,
                Level::Error,
                format!("mismatched port numbers ({} vs. {})", p1_number, p2_number),
            )
            .span_note(p2_loc, "conflicting port number is here")
            .matching_loc_note(matching_loc),
            SemanticError::WrongNumberOfTemplateArguments {
                expand_loc,
                def_loc,
                expand_length,
                def_length,
            } => {
                let arguments = if def_length == 1 {
                    "argument"
                } else {
                    "arguments"
                };
                Diagnostic::new(
                    expand_loc,
                    Level::Error,
                    format!(
                        "expected {} template {}, got {}",
                        def_length, arguments, expand_length
                    ),
                )
                .span_note(def_loc, "template defined here")
            }
            SemanticError::InvalidTemplateArg {
                param_name,
                arg_loc,
                param_loc,
                msg,
            } => Diagnostic::new(
                arg_loc,
                Level::Error,
                format!("invalid argument for template parameter {}: {}", param_name, msg),
            )
            .span_note(param_loc, "template parameter is defined here"),
            SemanticError::TemplateParameterConflict {
                param_name,
                param_loc,
                def_loc,
            } => Diagnostic::new(
                param_loc,
                Level::Error,
                format!(
                    "template parameter {} conflicts with a definition in the template body",
                    param_name
                ),
            )
            .span_note(def_loc, "conflicting definition is here"),
            SemanticError::NestedTemplateDefinition {
                nested_loc,
                parent_loc,
            } => Diagnostic::new(
                nested_loc,
                Level::Error,
                "module template definitions cannot be nested",
            )
            .span_note(parent_loc, "parent template defined here"),
            SemanticError::TemplateExpansionInTemplate {
                expand_loc,
                template_loc,
            } => Diagnostic::new(
                expand_loc,
                Level::Error,
                "template expansion specifiers cannot appear inside a module template",
            )
            .span_note(template_loc, "enclosing template defined here"),
            SemanticError::MissingAsync { kind, loc } => Diagnostic::new(
                loc,
                Level::Error,
                format!("{} component must have async input", kind),
            ),
            SemanticError::MissingConnection { loc, matching_loc } => {
                Diagnostic::new(loc, Level::Error, "no match for this connection")
                    .matching_loc_note(matching_loc)
            }
            SemanticError::MissingPort {
                loc,
                spec_msg,
                port_msg,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("component with {} must have {}", spec_msg, port_msg),
            ),
            SemanticError::MissingPortMatching { loc } => Diagnostic::new(
                loc,
                Level::Error,
                "unmatched connection must go from or to a matched port",
            ),
            SemanticError::NoPortAvailableForMatchedNumbering {
                loc1,
                loc2,
                matching_loc,
            } => Diagnostic::new(loc1, Level::Error, "no port available for matched numbering")
                .span_note(loc1, "matched connections are specified here")
                .span_note(loc2, "matched connections are specified here")
                .matching_loc_note(matching_loc)
                .note("to be available, a port number must be in bounds and unassigned at each of the matched ports"),
            SemanticError::OverlappingIdRanges {
                base_id1,
                name1,
                loc1,
                base_id2,
                max_id2,
                name2,
                loc2,
            } => Diagnostic::new(
                loc1,
                Level::Error,
                format!(
                    "base ID {} for instance {} lies inside the ID range [{}, {}] for instance {}",
                    base_id1, name1, base_id2, max_id2, name2
                ),
            )
            .span_note(loc2, format!("{} is defined here", name2)),
            SemanticError::InterfaceImport { import_loc, err } => {
                Diagnostic::new(import_loc, Level::Error, "failed to import interface")
                    .span_note_diagnostic((*err).into())
            }
            SemanticError::PortInterfaceMissingPort { loc } => {
                Diagnostic::new(loc, Level::Error, "port instance missing")
            }
            SemanticError::PortInterfaceInvalidPort { loc, def_loc } => Diagnostic::new(
                loc,
                Level::Error,
                "port instance does not match definition in interface",
            )
            .span_note(def_loc, "interface definition is here"),
            SemanticError::InterfaceImplements { loc, err } => {
                Diagnostic::new(loc, Level::Error, "port interface not implemented")
                    .span_note_diagnostic((*err).into())
            }
            SemanticError::PassiveAsync { loc, import_locs } => {
                let diag = Diagnostic::new(
                    loc,
                    Level::Error,
                    "passive component may not have async input",
                );
                import_locs.iter().rev().fold(diag, |diag, l| {
                    diag.span_note(*l, "port instance was imported from here")
                })
            }
            SemanticError::PassiveStateMachine { loc } => Diagnostic::new(
                loc,
                Level::Error,
                "passive component may not have a state machine instance",
            ),
            SemanticError::RedefinedSymbol {
                name,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("redefinition of symbol {}", name),
            )
            .span_note(prev_loc, "previous definition is here"),

            // State machine semantic errors
            SemanticError::CallSiteTypeMismatch {
                loc,
                te_kind,
                te_ty,
                site_kind,
                site_ty,
            } => Diagnostic::new(loc, Level::Error, format!("type mismatch at {}", te_kind))
                .note(format!("type of {} is {}", te_kind, te_ty))
                .note(format!("type of {} is {}", site_kind, site_ty)),
            SemanticError::DuplicateSignal {
                sig_name,
                state_name,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate use of signal {} in state {}", sig_name, state_name),
            )
            .span_note(prev_loc, "previous use is here"),
            SemanticError::InvalidInitialTransition { loc, msg, path, duplicate } => {
                let mut d = Diagnostic::new(loc, Level::Error, msg);
                for span in path {
                    d = d.span_note(span, "transition path goes here");
                }
                for span in duplicate {
                    d = d.span_note(span, "initial transition defined here");
                }
                d
            }
            SemanticError::ChoiceCycle { loc, path } => {
                let mut d = Diagnostic::new(loc, Level::Error, "encountered a choice cycle:");
                for (span, transition) in path {
                    d = d.span_note(span, transition);
                }
                d
            }
            SemanticError::ChoiceTypeMismatch {
                loc,
                to_loc1,
                to1,
                to_loc2,
                to2,
            } => Diagnostic::new(loc, Level::Error, "type mismatch at choice")
                .span_note(to_loc1, format!("type of transition is {}", to1))
                .span_note(to_loc2, format!("type of transition is {}", to2)),
            SemanticError::UnreachableNode { name, loc } => {
                Diagnostic::new(loc, Level::Error, format!("{} is unreachable", name))
            }
            // End of state machine semantic errors

            SemanticError::TooManyOutputPorts {
                loc,
                num_ports,
                array_size,
                instance_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!(
                    "too many ports connected here (found {}, max is {})",
                    num_ports, array_size
                ),
            )
            .span_note(instance_loc, "for this component instance"),
            SemanticError::TypeMismatch { loc, msg, reason } => {
                let diag = Diagnostic::new(loc, Level::Error, msg);
                match reason {
                    Some(reason) => reason.annotate(diag),
                    None => diag,
                }
            }
            SemanticError::UndefinedSymbol {
                name,
                symbol_kind,
                loc,
            } => Diagnostic::new(loc, Level::Error, format!("symbol {} is not defined", name))
                .note(format!("looking for a {} here", symbol_kind)),
            SemanticError::UseDefCycle {
                loc,
                use_def_matching_list,
            } => use_def_matching_list.iter().fold(
                Diagnostic::new(loc, Level::Error, "encountered a use-def cycle"),
                // Each definition on the path holds the next use, and the last
                // one is the location of the error, so the notes show the uses
                |out, m| out.span_note(m.node_id.span(), m.to_string()),
            ),
        }
    }
}
