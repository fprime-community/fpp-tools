use crate::semantics::TypeConversionError;
use fpp_core::{Diagnostic, Level, Span};

#[derive(Debug)]
pub struct SymbolUse {
    pub def_loc: Span,
    pub use_loc: Span,
}

/// A semantic error
#[derive(Debug)]
pub enum SemanticError {
    /// Redefined symbol
    RedefinedSymbol {
        /// Name of the symbol being redefined
        name: String,
        /// Location of the duplicate symbol
        loc: Span,
        /// Location of the previous symbol that is clashing
        prev_loc: Span,
    },
    /// Undefined symbol
    UndefinedSymbol {
        ng: String,
        name: String,
        loc: Span,
    },
    /// Use-def cycle
    UseDefCycle {
        loc: Span,
        cycle: Vec<SymbolUse>,
    },
    /// Invalid symbol
    InvalidSymbol {
        symbol_name: String,
        msg: String,
        loc: Span,
        def_loc: Span,
    },
    /// Invalid type
    InvalidType {
        loc: Span,
        msg: String,
    },
    /// Invalid qualifier
    InvalidQualifier {
        loc: Span,
        msg: String,
        def_loc: Span,
        def_msg: String,
    },
    /// Duplicate struct member
    DuplicateStructMember {
        name: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate parameter
    DuplicateParameter {
        name: String,
        loc: Span,
        prev_loc: Span,
    },
    TypeConversion {
        loc: Span,
        msg: String,
        err: Box<TypeConversionError>,
    },
    /// Empty array
    EmptyArray {
        loc: Span,
    },
    EnumConstantShouldBeImplied {
        loc: Span,
    },
    EnumConstantShouldBeExplicit {
        loc: Span,
    },
    /// Duplicate enum value
    DuplicateEnumConstant {
        value: i128,
        loc: Span,
        prev_loc: Span,
    },
    /// Invalid integer value
    InvalidIntValue {
        loc: Span,
        v: Option<i128>,
        msg: String,
    },
    /// Division by zero
    DivisionByZero {
        loc: Span,
    },
    /// Invalid shift amount
    InvalidShiftAmount {
        loc: Span,
    },
    /// A member-selection expression names a member that does not exist on the type
    InvalidTypeForMemberSelection {
        loc: Span,
        member: String,
        type_name: String,
    },
    FormatStringMismatchLength {
        format_locs: Vec<Span>,
        type_locs: Vec<Span>,
    },
    FormatStringInvalidReplacement {
        format_loc: Span,
        type_loc: Span,
        msg: String,
    },
    FormatStringInvalidPrecision {
        loc: Span,
        value: i32,
        max: i32,
    },
    ArrayDefaultMismatchedSize {
        loc: Span,
        size_loc: Span,
        value_size: usize,
        type_size: i128,
    },
    /// Invalid array size
    InvalidArraySize {
        loc: Span,
        size: i128,
    },
    /// Invalid priority specifier
    InvalidPriority {
        loc: Span,
    },
    /// Invalid queue full specifier
    InvalidQueueFull {
        loc: Span,
    },
    /// Invalid special port
    InvalidSpecialPort {
        loc: Span,
        msg: String,
    },
    /// Invalid port instance
    InvalidPortInstance {
        loc: Span,
        msg: String,
        def_loc: Span,
    },
    /// Duplicate interface
    DuplicateInterface {
        name: String,
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
    /// Error while importing an interface
    InterfaceImport {
        loc: Span,
        inner: Box<SemanticError>,
    },
    /// Passive async input
    PassiveAsync {
        loc: Span,
        import_locs: Vec<Span>,
    },
    /// Duplicate opcode value
    DuplicateOpcodeValue {
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
    /// Duplicate state machine instance
    DuplicateStateMachineInstance {
        name: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate telemetry channel limit
    DuplicateLimit {
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate name in dictionary
    DuplicateDictionaryName {
        kind: String,
        name: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate init specifier
    DuplicateInitSpecifier {
        phase: i128,
        loc: Span,
        prev_loc: Span,
    },
    /// Missing port
    MissingPort {
        loc: Span,
        spec_msg: String,
        port_msg: String,
    },
    /// Missing async input
    MissingAsync {
        kind: String,
        loc: Span,
    },
    PassiveStateMachine {
        loc: Span,
    },
    /// Invalid data products
    InvalidDataProducts {
        loc: Span,
        msg: String,
    },
    /// Invalid command
    InvalidCommand {
        loc: Span,
        msg: String,
    },
    /// Invalid event
    InvalidEvent {
        loc: Span,
        msg: String,
    },
    /// Invalid internal port
    InvalidInternalPort {
        loc: Span,
        msg: String,
    },
    /// Invalid port matching
    InvalidPortMatching {
        loc: Span,
        msg: String,
    },
    /// Invalid telemetry channel identifier name
    InvalidTlmChannelName {
        loc: Span,
        name: String,
        component_name: String,
    },
    /// Invalid component instance definition
    InvalidDefComponentInstance {
        name: String,
        loc: Span,
        msg: String,
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
    /// Duplicate instance
    DuplicateInstance {
        name: String,
        loc: Span,
        prev_loc: Span,
    },
    /// A model has more than one system definition.
    DuplicateSystemDefinition {
        loc: Span,
        prev_loc: Span,
    },
    /// A telemetry packet set specified in a non-deployment topology.
    InvalidTlmPacketSet {
        loc: Span,
        name: String,
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
    /// Invalid port instance identifier
    InvalidPortInstanceId {
        loc: Span,
        port_name: String,
        instance_type: String,
        interface_name: String,
    },
    /// Invalid port kind
    InvalidPortKind {
        loc: Span,
        msg: String,
        spec_loc: Span,
    },
    /// Invalid port number
    InvalidPortNumber {
        loc: Span,
        port_number: i128,
        port: String,
        array_size: i128,
        spec_loc: Span,
    },
    MissingPortMatching {
        loc: Span,
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
    /// Inconsistent location specifiers
    InconsistentLocationPath {
        /// Location of the first specifier's file string literal
        loc: Span,
        /// The path named by the first specifier
        path: String,
        /// Location of the second specifier's file string literal
        prev_loc: Span,
        /// The path named by the second specifier
        prev_path: String,
    },
    /// Incorrect dictionary specifier in location specifier
    IncorrectDictionarySpecifier {
        /// Location of the location specifier
        loc: Span,
        /// Location of the actual definition
        def_loc: Span,
    },
    /// Inconsistent dictionary specifiers in location specifiers
    InconsistentDictionarySpecifier {
        /// Location of the location specifier
        loc: Span,
        /// Location of the previous specifier
        prev_loc: Span,
    },
    /// Duplicate output port
    DuplicateOutputConnection {
        loc: Span,
        port_num: i128,
        prev_loc: Span,
    },
    /// Too many output ports
    TooManyOutputPorts {
        loc: Span,
        num_ports: i128,
        array_size: i128,
        instance_loc: Span,
    },
    /// Mismatched port numbers
    MismatchedPortNumbers {
        p1_loc: Span,
        p1_number: i128,
        p2_loc: Span,
        p2_number: i128,
        matching_loc: Span,
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
    /// Matched port numbering could not find a valid port number
    NoPortAvailableForMatchedNumbering {
        loc1: Span,
        loc2: Span,
        matching_loc: Span,
    },
    /// Missing connection
    MissingConnection {
        loc: Span,
        matching_loc: Span,
    },
    /// Duplicate matched connection
    DuplicateMatchedConnection {
        loc: Span,
        prev_loc: Span,
        matching_loc: Span,
    },
    /// Duplicate connection at matched port
    DuplicateConnectionAtMatchedPort {
        loc: Span,
        port: String,
        port_num: i128,
        prev_loc: Span,
        matching_loc: Span,
    },
    /// Invalid connection pattern
    InvalidPattern {
        loc: Span,
        msg: String,
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
        inner: Box<SemanticError>,
    },
    /// Duplicate pattern
    DuplicatePattern {
        kind: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Duplicate use of a signal in a state
    DuplicateSignal {
        name: String,
        state_name: String,
        loc: Span,
        prev_loc: Span,
    },
    /// Invalid initial transition
    InvalidInitialTransition {
        loc: Span,
        msg: String,
        /// Locations along the transition path (rendered as notes)
        path: Vec<Span>,
        /// Locations of duplicate initial transistions
        duplicate: Vec<Span>,
    },
    /// A cycle of choices in the transition graph
    ChoiceCycle {
        loc: Span,
        msg: String,
    },
    /// A node in the transition graph is unreachable
    UnreachableNode {
        name: String,
        loc: Span,
    },
    /// Type mismatch at a choice
    ChoiceTypeMismatch {
        loc: Span,
        loc1: Span,
        show1: String,
        loc2: Span,
        show2: String,
    },
    /// Type mismatch at a call site (action or guard)
    CallSiteTypeMismatch {
        loc: Span,
        te_kind: String,
        te_show: String,
        site_kind: String,
        site_show: String,
    },
}

/// The result of a compilation step
pub type SemanticResult<T = ()> = Result<T, SemanticError>;

impl SemanticError {
    /// Print the error
    pub fn emit(self) {
        Into::<Diagnostic>::into(self).emit();
    }
}

impl From<SemanticError> for Diagnostic {
    fn from(val: SemanticError) -> Self {
        match val {
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
            SemanticError::UndefinedSymbol { ng, name, loc } => Diagnostic::new(
                loc,
                Level::Error,
                format!("cannot find {} `{}` in scope", ng, name),
            ),
            SemanticError::InvalidSymbol {
                symbol_name,
                msg,
                loc,
                def_loc,
            } => Diagnostic::new(loc, Level::Error, msg)
                .span_note(def_loc, format!("{} defined here", symbol_name)),
            SemanticError::UseDefCycle { loc, cycle } => cycle.iter().enumerate().fold(
                Diagnostic::new(loc, Level::Error, "encountered symbol use-definition cycle"),
                |out, (i, suse)| match i {
                    0 => out.span_note(suse.def_loc, "defined here"),
                    _ if i == cycle.len() - 1 => out.span_note(suse.use_loc, "used here"),
                    _ => out
                        .span_note(suse.use_loc, "used here")
                        .span_note(suse.def_loc, "defined here"),
                },
            ),
            SemanticError::InvalidType { loc, msg } => Diagnostic::new(loc, Level::Error, msg),
            SemanticError::InvalidQualifier {
                loc,
                msg,
                def_loc,
                def_msg,
            } => Diagnostic::new(loc, Level::Error, msg).span_note(def_loc, def_msg),
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
            SemanticError::DuplicateParameter {
                name,
                loc,
                prev_loc,
            } => Diagnostic::new(loc, Level::Error, format!("duplicate parameter `{}`", name))
                .span_note(prev_loc, "previously defined here"),
            SemanticError::TypeConversion { loc, msg, err } => {
                err.annotate(Diagnostic::new(loc, Level::Error, msg))
            }
            SemanticError::EmptyArray { loc } => {
                Diagnostic::new(loc, Level::Error, "array expression may not be empty")
            }
            SemanticError::EnumConstantShouldBeImplied { loc } => {
                Diagnostic::new(loc, Level::Error, "expected constant value to be implied")
                    .note("enum constants must be all explicit or all implied")
            }
            SemanticError::EnumConstantShouldBeExplicit { loc } => {
                Diagnostic::new(loc, Level::Error, "expected constant value to be explicit")
                    .note("enum constants must be all explicit or all implied")
            }
            SemanticError::DuplicateEnumConstant {
                value,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate enum constant `{}`", value),
            )
            .span_note(prev_loc, "previously defined here"),
            SemanticError::InvalidIntValue { loc, v, msg } => {
                let diag = Diagnostic::new(loc, Level::Error, msg);
                match v {
                    None => diag,
                    Some(v) => diag.note(format!("expression evaluated to `{}`", v)),
                }
            }
            SemanticError::DivisionByZero { loc } => {
                Diagnostic::new(loc, Level::Error, "division by zero")
            }
            SemanticError::InvalidShiftAmount { loc } => {
                Diagnostic::new(loc, Level::Error, "invalid shift amount").note(
                    "shift amount must be a non-negative value that must be in the range [0,255]",
                )
            }
            SemanticError::InvalidTypeForMemberSelection {
                loc,
                member,
                type_name,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("{} has no member `{}`", type_name, member),
            ),
            SemanticError::FormatStringMismatchLength {
                format_locs,
                type_locs,
            } => {
                if format_locs.len() < type_locs.len() {
                    let diag = Diagnostic::new(
                        type_locs[format_locs.len()],
                        Level::Error,
                        "missing format replacement field",
                    );
                    type_locs[format_locs.len() + 1..]
                        .iter()
                        .fold(diag, |diag, loc| {
                            diag.span_note(*loc, "missing format replacement field")
                        })
                } else {
                    let diag = Diagnostic::new(
                        format_locs[type_locs.len()],
                        Level::Error,
                        "extraneous format replacement field",
                    );
                    format_locs[type_locs.len() + 1..]
                        .iter()
                        .fold(diag, |diag, loc| {
                            diag.span_annotation(*loc, "extraneous format replacement field")
                        })
                }
            }
            SemanticError::FormatStringInvalidReplacement {
                format_loc,
                type_loc,
                msg,
            } => Diagnostic::new(format_loc, Level::Error, msg)
                .span_note(type_loc, "type defined here"),
            SemanticError::FormatStringInvalidPrecision { loc, value, max } => Diagnostic::new(
                loc,
                Level::Error,
                format!(
                    "precision value `{}` is larger than the maximum ({})",
                    value, max
                ),
            ),
            SemanticError::ArrayDefaultMismatchedSize {
                loc,
                size_loc,
                value_size,
                type_size,
            } => Diagnostic::new(
                loc,
                Level::Error,
                "cannot convert value to array type due to mismatched sizes",
            )
            .note(format!("value size `{}`", value_size))
            .span_note(size_loc, format!("array size `{}`", type_size)),
            SemanticError::InvalidArraySize { loc, size } => Diagnostic::new(
                loc,
                Level::Error,
                format!("invalid array size {}", size),
            ),
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
            SemanticError::InvalidSpecialPort { loc, msg } => {
                Diagnostic::new(loc, Level::Error, msg)
            }
            SemanticError::InvalidPortInstance { loc, msg, def_loc } => {
                Diagnostic::new(loc, Level::Error, msg).span_note(def_loc, "defined here")
            }
            SemanticError::DuplicateInterface {
                name,
                loc,
                prev_loc,
            } => Diagnostic::new(loc, Level::Error, format!("duplicate interface {}", name))
                .span_note(prev_loc, "previous occurrence is here"),
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
                prev_import_locs
                    .iter()
                    .fold(diag, |diag, l| diag.span_note(*l, "port imported from here"))
            }
            SemanticError::InterfaceImport { loc, inner } => {
                let diag: Diagnostic = (*inner).into();
                diag.span_note(loc, "failed to import interface here")
            }
            SemanticError::PassiveAsync { loc, import_locs } => {
                let diag = Diagnostic::new(
                    loc,
                    Level::Error,
                    "passive component may not have async input",
                );
                import_locs.iter().fold(diag, |diag, l| {
                    diag.span_note(*l, "port instance was imported from here")
                })
            }
            SemanticError::DuplicateOpcodeValue {
                value,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate opcode value {}", value),
            )
            .span_note(prev_loc, "previous occurrence is here"),
            SemanticError::DuplicateIdValue {
                value,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate identifier value {}", value),
            )
            .span_note(prev_loc, "previous occurrence is here"),
            SemanticError::DuplicateStateMachineInstance {
                name,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate state machine instance {}", name),
            )
            .span_note(prev_loc, "previous occurrence is here"),
            SemanticError::DuplicateLimit { loc, prev_loc } => {
                Diagnostic::new(loc, Level::Error, "duplicate limit")
                    .span_note(prev_loc, "previous occurrence is here")
            }
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
            .span_note(prev_loc, "previous occurrence is here"),
            SemanticError::DuplicateInitSpecifier {
                phase,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate init specifier for phase {}", phase),
            )
            .span_note(prev_loc, "previous occurrence is here"),
            SemanticError::MissingPort {
                loc,
                spec_msg,
                port_msg,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("component with {} must have {}", spec_msg, port_msg),
            ),
            SemanticError::MissingAsync { kind, loc } => Diagnostic::new(
                loc,
                Level::Error,
                format!("{} component must have async input", kind),
            ),
            SemanticError::PassiveStateMachine { loc } => Diagnostic::new(
                loc,
                Level::Error,
                "passive component may not have state machine instances",
            ),
            SemanticError::InvalidDataProducts { loc, msg } => {
                Diagnostic::new(loc, Level::Error, msg)
            }
            SemanticError::InvalidCommand { loc, msg } => Diagnostic::new(loc, Level::Error, msg),
            SemanticError::InvalidEvent { loc, msg } => Diagnostic::new(loc, Level::Error, msg),
            SemanticError::InvalidInternalPort { loc, msg } => {
                Diagnostic::new(loc, Level::Error, msg)
            }
            SemanticError::InvalidPortMatching { loc, msg } => {
                Diagnostic::new(loc, Level::Error, msg)
            }
            SemanticError::InvalidTlmChannelName {
                loc,
                name,
                component_name,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!(
                    "{} is not a telemetry channel of component {}",
                    name, component_name
                ),
            ),
            SemanticError::InvalidDefComponentInstance { name, loc, msg } => {
                Diagnostic::new(loc, Level::Error, format!("invalid instance {}: {}", name, msg))
            }
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
                    "base id {} of instance {} is in the id range [{}, {}] of instance {}",
                    base_id1, name1, base_id2, max_id2, name2
                ),
            )
            .span_note(loc2, "conflicting instance is here"),
            SemanticError::DuplicateInstance {
                name,
                loc,
                prev_loc,
            } => Diagnostic::new(loc, Level::Error, format!("duplicate instance {}", name))
                .span_note(prev_loc, "previous instance is here"),
            SemanticError::DuplicateSystemDefinition { loc, prev_loc } => {
                Diagnostic::new(loc, Level::Error, "duplicate system definition")
                    .span_note(prev_loc, "previous definition is here")
                    .note("each model may have at most one system definition")
            }
            SemanticError::InvalidTlmPacketSet { loc, name, msg } => Diagnostic::new(
                loc,
                Level::Error,
                format!("invalid telemetry packet set {}", name),
            )
            .note(msg),
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
            SemanticError::InvalidPortInstanceId {
                loc,
                port_name,
                instance_type,
                interface_name,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!(
                    "{} is not a port instance of {} {}",
                    port_name, instance_type, interface_name
                ),
            ),
            SemanticError::InvalidPortKind { loc, msg, spec_loc } => {
                Diagnostic::new(loc, Level::Error, msg)
                    .span_note(spec_loc, "port instance is specified here")
            }
            SemanticError::InvalidPortNumber {
                loc,
                port_number,
                port,
                array_size,
                spec_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!(
                    "invalid port number {} for port {} (max is {})",
                    port_number,
                    port,
                    array_size - 1
                ),
            )
            .span_note(spec_loc, "port instance is specified here"),
            SemanticError::MissingPortMatching { loc } => Diagnostic::new(
                loc,
                Level::Error,
                "unmatched connection must go from or to a matched port",
            ),
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
            SemanticError::InconsistentDictionarySpecifier { loc, prev_loc } => {
                Diagnostic::new(loc, Level::Error, "inconsistent location specifier")
                    .span_note(prev_loc, "previous occurrence is here")
                    .note("one specifies dictionary and one does not")
            }
            SemanticError::DuplicateOutputConnection {
                loc,
                port_num,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate connection at output port {}", port_num),
            )
            .span_note(prev_loc, "previous occurrence is here"),
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
            .span_note(matching_loc, "port matching is specified here"),
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
            SemanticError::NoPortAvailableForMatchedNumbering {
                loc1,
                loc2,
                matching_loc,
            } => Diagnostic::new(loc1, Level::Error, "no port available for matched numbering")
                .span_note(loc1, "matched connections are specified here")
                .span_note(loc2, "matched connections are specified here")
                .span_note(matching_loc, "port matching is specified here")
                .note("to be available, a port number must be in bounds and unassigned at each of the matched ports"),
            SemanticError::MissingConnection { loc, matching_loc } => {
                Diagnostic::new(loc, Level::Error, "no match for this connection")
                    .span_note(matching_loc, "port matching is specified here")
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
            .span_note(prev_loc, "previous occurrence is here")
            .span_note(matching_loc, "port matching is specified here")
            .note("each port in a matched port array must be connected to a separate instance"),
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
            .span_note(prev_loc, "previous occurrence is here")
            .span_note(matching_loc, "port matching is specified here"),
            SemanticError::InvalidPattern { loc, msg } => Diagnostic::new(loc, Level::Error, msg),
            SemanticError::PortInterfaceMissingPort { loc } => {
                Diagnostic::new(loc, Level::Error, "port instance missing")
            }
            SemanticError::PortInterfaceInvalidPort { loc, def_loc } => Diagnostic::new(
                loc,
                Level::Error,
                "port instance does not match definition in interface",
            )
            .span_note(def_loc, "interface definition is here"),
            SemanticError::InterfaceImplements { loc, inner } => {
                let d = Diagnostic::new(loc, Level::Error, "port interface not implemented");
                match *inner {
                    SemanticError::PortInterfaceMissingPort { loc: iloc } => {
                        d.span_note(iloc, "port instance missing")
                    }
                    SemanticError::PortInterfaceInvalidPort {
                        loc: iloc,
                        def_loc,
                    } => d
                        .span_note(iloc, "port instance does not match definition in interface")
                        .span_note(def_loc, "interface definition is here"),
                    other => d.span_note(loc, format!("{:?}", other)),
                }
            }
            SemanticError::DuplicatePattern {
                kind,
                loc,
                prev_loc,
            } => Diagnostic::new(loc, Level::Error, format!("duplicate {} pattern", kind))
                .span_note(prev_loc, "previous occurrence is here"),
            SemanticError::DuplicateSignal {
                name,
                state_name,
                loc,
                prev_loc,
            } => Diagnostic::new(
                loc,
                Level::Error,
                format!("duplicate use of signal {} in state {}", name, state_name),
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
            SemanticError::ChoiceCycle { loc, msg } => Diagnostic::new(loc, Level::Error, msg),
            SemanticError::UnreachableNode { name, loc } => {
                Diagnostic::new(loc, Level::Error, format!("{} is unreachable", name))
            }
            SemanticError::ChoiceTypeMismatch {
                loc,
                loc1,
                show1,
                loc2,
                show2,
            } => Diagnostic::new(loc, Level::Error, "type mismatch at choice")
                .span_note(loc1, format!("type of transition is {}", show1))
                .span_note(loc2, format!("type of transition is {}", show2)),
            SemanticError::CallSiteTypeMismatch {
                loc,
                te_kind,
                te_show,
                site_kind,
                site_show,
            } => Diagnostic::new(loc, Level::Error, format!("type mismatch at {}", te_kind))
                .note(format!("type of {} is {}", te_kind, te_show))
                .note(format!("type of {} is {}", site_kind, site_show)),
        }
    }
}
