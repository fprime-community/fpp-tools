use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    Format, GeneralPortInstance, PortInstance, PortInterface, SpecialPortInstance, Symbol,
    SymbolInterface, Type, Value, state_machine,
};
use fpp_ast::{
    AstNode, ComponentKind, DefComponent, Ident, InputPortKind, QueueFull, SpecCommand,
    SpecContainer, SpecEvent, SpecParam, SpecPortMatching, SpecRecord, SpecStateMachineInstance,
    SpecTlmChannel, SpecialPortInstanceKind, TlmChannelLimitKind, TlmChannelUpdate,
};
use fpp_core::{Node, Span, Spanned};
use rustc_hash::FxHashMap as HashMap;
use std::sync::Arc;

/// Display an id value as `(<dec> dec, <HEX> hex)`.
pub fn display_id_value(v: i128) -> String {
    format!("({} dec, {:X} hex)", v, v)
}

/// An FPP command.
#[derive(Debug, Clone)]
pub enum Command {
    /// A non-parameter command.
    NonParam {
        node: Arc<SpecCommand>,
        kind: NonParamKind,
    },
    /// A parameter command, i.e. the set or save command implied by a
    /// parameter specifier.
    Param {
        node: Arc<SpecParam>,
        kind: ParamKind,
    },
}

/// The kind of a non-parameter command.
#[derive(Debug, Clone)]
pub enum NonParamKind {
    Async {
        priority: Option<i128>,
        queue_full: QueueFull,
    },
    Guarded,
    Sync,
}

/// The kind of a parameter command.
#[derive(Debug, Clone)]
pub enum ParamKind {
    Save,
    Set,
}

impl Command {
    /// Gets the location of the command.
    pub fn get_loc(&self) -> Span {
        match self {
            Command::NonParam { node, .. } => node.span(),
            Command::Param { node, .. } => node.span(),
        }
    }

    /// Gets the name of the command.
    pub fn get_name(&self) -> String {
        match self {
            Command::NonParam { node, .. } => node.name.data.clone(),
            Command::Param { node, kind } => {
                let param_name = node.name.data.to_uppercase();
                match kind {
                    ParamKind::Save => format!("{}_PRM_SAVE", param_name),
                    ParamKind::Set => format!("{}_PRM_SET", param_name),
                }
            }
        }
    }

    pub fn is_async(&self) -> bool {
        matches!(
            self,
            Command::NonParam {
                kind: NonParamKind::Async { .. },
                ..
            }
        )
    }

    /// Creates a command from a command specifier.
    pub fn from_spec_command(a: &Analysis, node: &SpecCommand) -> SemanticResult<Command> {
        let loc = node.span();
        if !matches!(node.kind, InputPortKind::Async) {
            if let Some(priority) = &node.priority {
                return Err(SemanticError::InvalidPriority {
                    loc: priority.span(),
                });
            }
            if let Some(queue_full) = &node.queue_full {
                return Err(SemanticError::InvalidQueueFull {
                    loc: queue_full.span(),
                });
            }
        }
        let priority = a.get_big_int_value_opt(&node.priority);
        Analysis::check_for_duplicate_parameter(&node.params)?;
        if Analysis::get_num_ref_params(&node.params) != 0 {
            return Err(SemanticError::InvalidCommand {
                loc,
                msg: "command may not have ref parameters".to_string(),
            });
        }
        a.check_displayable_params(&node.params, "type of command parameter is not displayable")?;
        let kind = match node.kind {
            InputPortKind::Async => NonParamKind::Async {
                priority,
                queue_full: Analysis::get_specified_queue_full(&node.queue_full),
            },
            InputPortKind::Guarded => NonParamKind::Guarded,
            InputPortKind::Sync => NonParamKind::Sync,
        };
        Ok(Command::NonParam {
            node: Arc::new(node.clone()),
            kind,
        })
    }
}

/// A map from limit kinds to the node and value of the limit.
pub type Limits = HashMap<TlmChannelLimitKind, (Node, Value)>;

/// A telemetry channel.
#[derive(Debug, Clone)]
pub struct TlmChannel {
    pub node: Arc<SpecTlmChannel>,
    pub channel_type: Arc<Type>,
    pub update: TlmChannelUpdate,
    pub format: Option<Format>,
    pub low_limits: Limits,
    pub high_limits: Limits,
}

impl TlmChannel {
    /// Gets the name of the channel.
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the channel.
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Creates a telemetry channel from a telemetry channel specifier.
    pub fn from_spec(a: &Analysis, node: &SpecTlmChannel) -> SemanticResult<TlmChannel> {
        let channel_type = a.get_finalized_type(node.type_name.node_id).unwrap();
        let update = node.update.clone().unwrap_or(TlmChannelUpdate::Always);
        let format = node
            .format
            .as_ref()
            .map(|format| Format::new(format, vec![(channel_type.clone(), node.type_name.span())]));
        let low_limits = compute_limits(a, &node.low)?;
        let high_limits = compute_limits(a, &node.high)?;
        a.check_displayable_type(
            node.type_name.node_id,
            node.type_name.span(),
            "type of telemetry channel is not displayable",
        )?;
        Ok(TlmChannel {
            node: Arc::new(node.clone()),
            channel_type,
            update,
            format,
            low_limits,
            high_limits,
        })
    }
}

/// Computes limits from AST limits.
fn compute_limits(a: &Analysis, limits: &[fpp_ast::TlmChannelLimit]) -> SemanticResult<Limits> {
    let mut result = Limits::default();
    // The kinds seen so far. Kept separately from `result`, because a limit
    // whose expression has no value is still a duplicate of one that does.
    let mut seen: HashMap<TlmChannelLimitKind, Node> = HashMap::default();
    for limit in limits {
        if let Some(prev_node) = seen.insert(limit.kind.clone(), limit.node_id) {
            return Err(SemanticError::DuplicateLimit {
                loc: limit.span(),
                prev_loc: prev_node.span(),
            });
        }
        // A limit expression with no value had its error reported by an
        // earlier pass; omit it from the map.
        if let Some(value) = a.value_map.get(&limit.value.node_id) {
            result.insert(limit.kind.clone(), (limit.node_id, value.clone()));
        }
    }
    Ok(result)
}

/// A data product record.
#[derive(Debug, Clone)]
pub struct Record {
    pub node: Arc<SpecRecord>,
    pub record_type: Arc<Type>,
    pub is_array: bool,
}

impl Record {
    /// Gets the name of the record.
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the record.
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Creates a record from a record specifier.
    pub fn from_spec(a: &Analysis, node: &SpecRecord) -> SemanticResult<Record> {
        let record_type = a.get_finalized_type(node.record_type.node_id).unwrap();
        a.check_displayable_type(
            node.record_type.node_id,
            node.record_type.span(),
            "type of record is not displayable",
        )?;
        Ok(Record {
            node: Arc::new(node.clone()),
            record_type,
            is_array: node.is_array,
        })
    }
}

/// A data product container.
#[derive(Debug, Clone)]
pub struct Container {
    pub node: Arc<SpecContainer>,
    pub default_priority: Option<i128>,
}

impl Container {
    /// Gets the name of the container.
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the container.
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Creates a container from a container specifier.
    pub fn from_spec(a: &Analysis, node: &SpecContainer) -> SemanticResult<Container> {
        let default_priority = a.get_nonnegative_big_int_value_opt(&node.default_priority)?;
        Ok(Container {
            node: Arc::new(node.clone()),
            default_priority,
        })
    }
}

/// A parameter.
#[derive(Debug, Clone)]
pub struct Param {
    pub node: Arc<SpecParam>,
    pub param_type: Arc<Type>,
    pub default: Option<Value>,
    pub set_opcode: i128,
    pub save_opcode: i128,
    pub is_external: bool,
}

impl Param {
    /// Gets the name of the parameter.
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the parameter.
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Create a parameter, returning it plus the updated default opcode.
    pub fn from_spec(
        a: &Analysis,
        node: &SpecParam,
        default_opcode: i128,
    ) -> SemanticResult<(Param, i128)> {
        let param_type = a.get_finalized_type(node.type_name.node_id).unwrap();
        let mut default = None;
        if let Some(default_node) = &node.default {
            if let Some(default_ty) = a.type_map.get(&default_node.node_id)
                && let Err(err) = Type::convert(default_ty, &param_type)
            {
                return Err(SemanticError::TypeConversion {
                    loc: default_node.span(),
                    msg: format!("default value cannot be converted to {}", param_type),
                    err: Box::new(err),
                });
            }
            default = a
                .value_map
                .get(&default_node.node_id)
                .and_then(|v| v.convert(&param_type));
        }
        let set_opcode_opt = a.get_nonnegative_big_int_value_opt(&node.set_opcode)?;
        let save_opcode_opt = a.get_nonnegative_big_int_value_opt(&node.save_opcode)?;
        a.check_displayable_type(
            node.type_name.node_id,
            node.type_name.span(),
            "type of parameter is not displayable",
        )?;
        let (set_opcode, default1) = compute_opcode(set_opcode_opt, default_opcode);
        let (save_opcode, default2) = compute_opcode(save_opcode_opt, default1);
        Ok((
            Param {
                node: Arc::new(node.clone()),
                param_type,
                default,
                set_opcode,
                save_opcode,
                is_external: node.is_external,
            },
            default2,
        ))
    }
}

fn compute_opcode(int_opt: Option<i128>, default_opcode: i128) -> (i128, i128) {
    match int_opt {
        Some(i) => (i, default_opcode),
        None => (default_opcode, default_opcode + 1),
    }
}

/// A time interval, as used in an event throttle.
#[derive(Debug, Clone)]
pub struct TimeInterval {
    pub seconds: i64,
    pub useconds: i32,
}

/// An event throttle.
#[derive(Debug, Clone)]
pub struct Throttle {
    pub count: i32,
    pub every: Option<TimeInterval>,
}

/// An event.
#[derive(Debug, Clone)]
pub struct Event {
    pub node: Arc<SpecEvent>,
    pub format: Format,
    pub throttle: Option<Throttle>,
}

impl Event {
    /// Gets the name of the event.
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the event.
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Creates an event from an event specifier.
    pub fn from_spec(a: &Analysis, node: &SpecEvent) -> SemanticResult<Event> {
        let loc = node.span();
        if Analysis::get_num_ref_params(&node.params) != 0 {
            return Err(SemanticError::InvalidEvent {
                loc,
                msg: "event may not have ref parameters".to_string(),
            });
        }
        a.check_displayable_params(&node.params, "type of event is not displayable")?;
        let types: Vec<(Arc<Type>, Span)> = node
            .params
            .iter()
            .filter_map(|p| {
                a.type_map
                    .get(&p.type_name.node_id)
                    .map(|t| (t.clone(), p.type_name.span()))
            })
            .collect();
        let format = Format::new(&node.format, types);
        let throttle = match &node.throttle {
            Some(throttle) => Some(check_event_throttle(a, throttle, loc)?),
            None => None,
        };
        Ok(Event {
            node: Arc::new(node.clone()),
            format,
            throttle,
        })
    }
}

fn check_event_throttle(
    a: &Analysis,
    throttle: &fpp_ast::EventThrottle,
    loc: Span,
) -> SemanticResult<Throttle> {
    let count = a.get_nonnegative_int_value(throttle.count.node_id, throttle.count.span())?;
    if count == 0 {
        return Err(SemanticError::InvalidEvent {
            loc,
            msg: "event throttle count must be greater than zero".to_string(),
        });
    }
    let every = match &throttle.every {
        Some(every) => check_throttle_interval(a, every)?,
        None => None,
    };
    Ok(Throttle {
        count: count as i32,
        every,
    })
}

/// Computes the time interval of an event throttle.
///
/// Returns `None` when the interval expression has no value of the shape
/// `{ seconds: U32, useconds: U32 }`. That can only happen on an expression
/// whose error CheckExprTypes already reported, so there is nothing to add here;
/// the only diagnostics from here are the range checks.
fn check_throttle_interval(
    a: &Analysis,
    every: &fpp_ast::Expr,
) -> SemanticResult<Option<TimeInterval>> {
    let loc = every.span();
    use crate::semantics::{AnonStructType, StructValue};
    let u32_ty = Arc::new(Type::PrimitiveInt(fpp_ast::IntegerKind::U32));
    let mut members = HashMap::default();
    members.insert("seconds".to_string(), u32_ty.clone());
    members.insert("useconds".to_string(), u32_ty.clone());
    let interval_ty = Arc::new(Type::AnonStruct(AnonStructType { members }));

    let Some(value) = a.value_map.get(&every.node_id) else {
        return Ok(None);
    };
    let interval = match value.convert(&interval_ty) {
        Some(Value::Struct(StructValue { anon_struct, .. }))
        | Some(Value::AnonStruct(anon_struct)) => anon_struct,
        _ => return Ok(None),
    };

    let Some(seconds) = check_interval_member(&interval, "seconds", u32::MAX as i128, loc)? else {
        return Ok(None);
    };
    let Some(useconds) = check_interval_member(&interval, "useconds", 999_999, loc)? else {
        return Ok(None);
    };
    Ok(Some(TimeInterval {
        seconds: seconds as i64,
        useconds: useconds as i32,
    }))
}

/// Checks one member of a throttle interval against its range. Returns `None`
/// if the member is absent or not an integer; see `check_throttle_interval`.
fn check_interval_member(
    interval: &crate::semantics::AnonStructValue,
    member: &str,
    max_value: i128,
    loc: Span,
) -> SemanticResult<Option<i128>> {
    use crate::semantics::{IntegerValue, PrimitiveIntegerValue};
    let u32_ty = Arc::new(Type::PrimitiveInt(fpp_ast::IntegerKind::U32));
    let v = match interval
        .members
        .get(member)
        .and_then(|v| v.convert(&u32_ty))
    {
        Some(Value::PrimitiveInteger(PrimitiveIntegerValue { value, .. }))
        | Some(Value::Integer(IntegerValue(value))) => value,
        _ => return Ok(None),
    };
    if v < 0 || v > max_value {
        return Err(SemanticError::InvalidIntValue {
            loc,
            v: Some(v),
            msg: format!("{} must be in the range [0, {}]", member, max_value),
        });
    }
    Ok(Some(v))
}

/// A state machine instance.
#[derive(Debug, Clone)]
pub struct StateMachineInstance {
    pub node: Arc<SpecStateMachineInstance>,
    pub state_machine: Arc<fpp_ast::DefStateMachine>,
    pub priority: Option<i128>,
    pub queue_full: QueueFull,
}

impl StateMachineInstance {
    /// Gets the location of the state machine instance.
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Gets the node ID.
    pub fn get_node_id(&self) -> Node {
        self.node.node_id
    }

    /// Gets the name of the state machine instance.
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the symbol of the state machine this is an instance of.
    pub fn symbol(&self) -> Symbol {
        Symbol::StateMachine(self.state_machine.clone())
    }

    /// Gets the state machine kind.
    pub fn get_sm_kind(&self) -> state_machine::Kind {
        state_machine::StateMachine::get_symbol_kind(&self.state_machine)
    }

    pub fn from_spec(
        a: &Analysis,
        node: &SpecStateMachineInstance,
    ) -> SemanticResult<Option<StateMachineInstance>> {
        let priority = a.get_big_int_value_opt(&node.priority);
        let queue_full = Analysis::get_queue_full(&node.queue_full);
        let state_machine = match a.use_def_map.get(&node.state_machine.id()) {
            Some(Symbol::StateMachine(sm)) => sm.clone(),
            Some(symbol) => {
                return Err(SemanticError::InvalidSymbol {
                    symbol_name: symbol.name().data.clone(),
                    loc: node.state_machine.span(),
                    msg: "not a state machine symbol".to_string(),
                    def_loc: symbol.name().span(),
                });
            }
            // Unresolved use: CheckUses already reported the error.
            None => return Ok(None),
        };
        Ok(Some(StateMachineInstance {
            node: Arc::new(node.clone()),
            state_machine,
            priority,
            queue_full,
        }))
    }
}

/// An FPP component.
#[derive(Debug, Clone)]
pub struct Component {
    pub symbol: Symbol,
    /// The AST node defining the component.
    pub node: Arc<DefComponent>,
    /// The port interface of the component.
    pub port_interface: PortInterface,
    /// The map from command opcodes to commands.
    pub command_map: HashMap<i128, Command>,
    /// The next default opcode.
    pub default_opcode: i128,
    /// The map from telemetry channel IDs to channels.
    pub tlm_channel_map: HashMap<i128, TlmChannel>,
    /// The map from telemetry channel names to channels.
    pub tlm_channel_name_map: HashMap<String, TlmChannel>,
    /// The next default channel ID.
    pub default_tlm_channel_id: i128,
    /// The map from event IDs to events.
    pub event_map: HashMap<i128, Event>,
    /// The next default event ID.
    pub default_event_id: i128,
    /// The map from parameter IDs to parameters.
    pub param_map: HashMap<i128, Param>,
    /// The next default parameter ID.
    pub default_param_id: i128,
    /// The map from container IDs to containers.
    pub container_map: HashMap<i128, Container>,
    /// The next default container ID.
    pub default_container_id: i128,
    /// The map from record IDs to records.
    pub record_map: HashMap<i128, Record>,
    /// The next default record ID.
    pub default_record_id: i128,
    /// The map from state machine instance names to state machine instances.
    pub state_machine_instance_map: HashMap<String, StateMachineInstance>,
    /// The list of port matching specifiers.
    pub spec_port_matching_list: Vec<Arc<SpecPortMatching>>,
    /// The resolved port matchings of this component. Populated with the
    /// matched-port-numbering phase; empty otherwise.
    pub port_matching_list: Vec<PortMatching>,
}

/// A port matching. Only a general port may be matched.
#[derive(Debug, Clone)]
pub struct PortMatching {
    pub node: Arc<SpecPortMatching>,
    pub instance1: GeneralPortInstance,
    pub instance2: GeneralPortInstance,
}

impl PortMatching {
    /// Gets the location of the port matching.
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Whether the given port instance participates in this matching.
    pub fn matches(&self, pi: &PortInstance) -> bool {
        self.instance1.get_node_id() == pi.get_node_id()
            || self.instance2.get_node_id() == pi.get_node_id()
    }
}

impl std::fmt::Display for PortMatching {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "match {} with {}", self.instance1, self.instance2)
    }
}

impl Component {
    pub fn new(symbol: Symbol, node: Arc<DefComponent>) -> Component {
        Component {
            symbol,
            node,
            port_interface: PortInterface::new("component"),
            command_map: HashMap::default(),
            default_opcode: 0,
            tlm_channel_map: HashMap::default(),
            tlm_channel_name_map: HashMap::default(),
            default_tlm_channel_id: 0,
            event_map: HashMap::default(),
            default_event_id: 0,
            param_map: HashMap::default(),
            default_param_id: 0,
            container_map: HashMap::default(),
            default_container_id: 0,
            record_map: HashMap::default(),
            default_record_id: 0,
            state_machine_instance_map: HashMap::default(),
            spec_port_matching_list: vec![],
            port_matching_list: vec![],
        }
    }

    /// The map from port names to port instances.
    pub fn port_map(&self) -> &HashMap<String, PortInstance> {
        &self.port_interface.port_map
    }

    /// The map from special port kinds to special port instances.
    pub fn special_port_map(&self) -> &HashMap<SpecialPortInstanceKind, SpecialPortInstance> {
        &self.port_interface.special_port_map
    }

    fn kind(&self) -> &ComponentKind {
        &self.node.kind
    }

    fn component_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the component
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Query whether the component has parameters
    pub fn has_parameters(&self) -> bool {
        !self.param_map.is_empty()
    }
    /// Query whether the component has external parameters
    pub fn has_external_parameters(&self) -> bool {
        self.param_map.values().any(|p| p.is_external)
    }
    /// Query whether the component has commands
    pub fn has_commands(&self) -> bool {
        !self.command_map.is_empty()
    }
    /// Query whether the component has events
    pub fn has_events(&self) -> bool {
        !self.event_map.is_empty()
    }
    /// Query whether the component has telemetry
    pub fn has_telemetry(&self) -> bool {
        !self.tlm_channel_map.is_empty()
    }
    /// Query whether the component has data products
    pub fn has_data_products(&self) -> bool {
        !self.record_map.is_empty() || !self.container_map.is_empty()
    }
    /// Query whether the component has state machine instances
    pub fn has_state_machine_instances(&self) -> bool {
        !self.state_machine_instance_map.is_empty()
    }
    /// Query whether the component has state machine instances of the
    /// specified kind
    pub fn has_state_machine_instances_of_kind(&self, kind: &state_machine::Kind) -> bool {
        self.state_machine_instance_map
            .values()
            .any(|i| &i.get_sm_kind() == kind)
    }

    /// Gets a telemetry channel by name
    pub fn get_tlm_channel_by_name(&self, name: &Ident) -> SemanticResult<TlmChannel> {
        match self.tlm_channel_name_map.get(&name.data) {
            Some(tlm_channel) => Ok(tlm_channel.clone()),
            None => Err(SemanticError::InvalidTlmChannelName {
                loc: name.span(),
                name: name.data.clone(),
                component_name: self.component_name().to_string(),
            }),
        }
    }

    /// Gets the max identifier
    pub fn get_max_id(&self) -> i128 {
        fn max_in_map<T>(map: &HashMap<i128, T>) -> i128 {
            map.keys().copied().max().unwrap_or(-1)
        }
        [
            max_in_map(&self.command_map),
            max_in_map(&self.container_map),
            max_in_map(&self.event_map),
            max_in_map(&self.param_map),
            max_in_map(&self.tlm_channel_map),
        ]
        .into_iter()
        .max()
        .unwrap_or(-1)
    }

    /// Add a command
    pub fn add_command(&mut self, opcode_opt: Option<i128>, command: Command) -> SemanticResult {
        let opcode = opcode_opt.unwrap_or(self.default_opcode);
        if let Some(prev) = self.command_map.get(&opcode) {
            return Err(SemanticError::DuplicateOpcodeValue {
                value: display_id_value(opcode),
                loc: command.get_loc(),
                prev_loc: prev.get_loc(),
            });
        }
        self.command_map.insert(opcode, command);
        self.default_opcode = opcode + 1;
        Ok(())
    }

    /// Add a state machine instance
    pub fn add_state_machine_instance(&mut self, instance: StateMachineInstance) -> SemanticResult {
        let name = instance.get_name().to_string();
        if let Some(prev) = self.state_machine_instance_map.get(&name) {
            return Err(SemanticError::DuplicateStateMachineInstance {
                name,
                loc: instance.get_loc(),
                prev_loc: prev.get_loc(),
            });
        }
        self.state_machine_instance_map.insert(name, instance);
        Ok(())
    }

    /// Add a data product container
    pub fn add_container(&mut self, id_opt: Option<i128>, container: Container) -> SemanticResult {
        let next = add_element_to_id_map(
            &mut self.container_map,
            id_opt.unwrap_or(self.default_container_id),
            container,
            Container::get_loc,
        )?;
        self.default_container_id = next;
        Ok(())
    }

    /// Add an event
    pub fn add_event(&mut self, id_opt: Option<i128>, event: Event) -> SemanticResult {
        let next = add_element_to_id_map(
            &mut self.event_map,
            id_opt.unwrap_or(self.default_event_id),
            event,
            Event::get_loc,
        )?;
        self.default_event_id = next;
        Ok(())
    }

    /// Add a data product record
    pub fn add_record(&mut self, id_opt: Option<i128>, record: Record) -> SemanticResult {
        let next = add_element_to_id_map(
            &mut self.record_map,
            id_opt.unwrap_or(self.default_record_id),
            record,
            Record::get_loc,
        )?;
        self.default_record_id = next;
        Ok(())
    }

    /// Add a telemetry channel
    pub fn add_tlm_channel(&mut self, id_opt: Option<i128>, channel: TlmChannel) -> SemanticResult {
        let name = channel.get_name().to_string();
        let next = add_element_to_id_map(
            &mut self.tlm_channel_map,
            id_opt.unwrap_or(self.default_tlm_channel_id),
            channel.clone(),
            TlmChannel::get_loc,
        )?;
        // Add the channel to the channel name map. If there is a duplicate
        // name, we will catch it later when we check all the dictionary
        // elements.
        self.tlm_channel_name_map.insert(name, channel);
        self.default_tlm_channel_id = next;
        Ok(())
    }

    /// Add a parameter
    pub fn add_param(&mut self, id_opt: Option<i128>, param: Param) -> SemanticResult {
        let set_opcode = param.set_opcode;
        let save_opcode = param.save_opcode;
        // The implicit set and save commands share the parameter's spec node.
        let set_command = Command::Param {
            node: param.node.clone(),
            kind: ParamKind::Set,
        };
        let save_command = Command::Param {
            node: param.node.clone(),
            kind: ParamKind::Save,
        };
        // Update the parameter map and the default parameter ID.
        let next = add_element_to_id_map(
            &mut self.param_map,
            id_opt.unwrap_or(self.default_param_id),
            param,
            Param::get_loc,
        )?;
        self.default_param_id = next;
        // Add the implicit set and save commands.
        self.add_command(Some(set_opcode), set_command)?;
        self.add_command(Some(save_opcode), save_command)?;
        Ok(())
    }

    /// Add a port instance
    pub fn add_port_instance(&mut self, instance: PortInstance) -> SemanticResult {
        let pi = self.port_interface.add_port_instance(instance)?;
        self.port_interface = pi;
        Ok(())
    }

    pub fn add_imported_interface(
        &mut self,
        interface: &crate::semantics::Interface,
        import_node: Node,
    ) -> SemanticResult {
        let pi = self
            .port_interface
            .add_imported_interface(interface, import_node)?;
        self.port_interface = pi;
        Ok(())
    }

    pub fn add_spec_port_matching(&mut self, node: Arc<SpecPortMatching>) {
        self.spec_port_matching_list.insert(0, node);
    }

    /// Complete a component definition.
    pub fn complete(mut self) -> SemanticResult<Component> {
        self.port_matching_list = self.construct_port_matching_list()?;
        self.check_validity()?;
        Ok(self)
    }

    /// Checks whether a component is valid
    fn check_validity(&self) -> SemanticResult {
        self.check_no_duplicate_names()?;
        match self.kind() {
            ComponentKind::Passive => self.check_no_async_input()?,
            _ => self.check_async_input()?,
        }
        self.check_required_ports()?;
        self.check_data_products()?;
        Ok(())
    }

    /// Checks that there are no duplicate names in dictionaries
    fn check_no_duplicate_names(&self) -> SemanticResult {
        check_dictionary_names(
            &self.param_map,
            "parameter",
            |p| p.get_name().to_string(),
            Param::get_loc,
        )?;
        check_dictionary_names(
            &self.command_map,
            "command",
            Command::get_name,
            Command::get_loc,
        )?;
        check_dictionary_names(
            &self.event_map,
            "event",
            |e| e.get_name().to_string(),
            Event::get_loc,
        )?;
        check_dictionary_names(
            &self.tlm_channel_map,
            "telemetry channel",
            |t| t.get_name().to_string(),
            TlmChannel::get_loc,
        )?;
        check_dictionary_names(
            &self.container_map,
            "container",
            |c| c.get_name().to_string(),
            Container::get_loc,
        )?;
        check_dictionary_names(
            &self.record_map,
            "record",
            |r| r.get_name().to_string(),
            Record::get_loc,
        )?;
        Ok(())
    }

    /// Checks that component has at least one async input port or async command
    fn check_async_input(&self) -> SemanticResult {
        if self.check_no_async_input().is_err() {
            Ok(())
        } else {
            Err(SemanticError::MissingAsync {
                kind: self.kind().to_string(),
                loc: self.get_loc(),
            })
        }
    }

    /// Checks that component has no async input ports
    fn check_no_async_input(&self) -> SemanticResult {
        for instance in self.port_map().values() {
            if instance.is_async_input() {
                return Err(SemanticError::PassiveAsync {
                    loc: instance.get_loc(),
                    import_locs: instance.get_import_locs().to_vec(),
                });
            }
        }
        for command in self.command_map.values() {
            if command.is_async() {
                return Err(SemanticError::PassiveAsync {
                    loc: command.get_loc(),
                    import_locs: vec![],
                });
            }
        }
        if let Some(instance) = self.state_machine_instance_map.values().next() {
            return Err(SemanticError::PassiveStateMachine {
                loc: instance.get_loc(),
            });
        }
        Ok(())
    }

    fn has_special_port(&self, kind: &SpecialPortInstanceKind) -> bool {
        self.special_port_map().contains_key(kind)
    }

    /// Check that component provides ports required by dictionary
    /// and data product specifiers
    fn check_required_ports(&self) -> SemanticResult {
        use SpecialPortInstanceKind::*;
        let require = |condition: bool,
                       spec_msg: &str,
                       kinds: &[SpecialPortInstanceKind]|
         -> SemanticResult {
            if condition {
                for kind in kinds {
                    if !self.has_special_port(kind) {
                        return Err(SemanticError::MissingPort {
                            loc: self.get_loc(),
                            spec_msg: spec_msg.to_string(),
                            port_msg: format!("{} port", kind),
                        });
                    }
                }
            }
            Ok(())
        };
        require(
            self.has_parameters(),
            "parameter specifiers",
            &[ParamGet, ParamSet, CommandRecv, CommandReg, CommandResp],
        )?;
        require(
            self.has_commands(),
            "command specifiers",
            &[CommandRecv, CommandReg, CommandResp],
        )?;
        require(
            self.has_events(),
            "event specifiers",
            &[Event, TextEvent, TimeGet],
        )?;
        require(
            self.has_telemetry(),
            "telemetry specifiers",
            &[Telemetry, TimeGet],
        )?;
        if self.has_data_products()
            && !self.has_special_port(&ProductGet)
            && !self.has_special_port(&ProductRequest)
        {
            return Err(SemanticError::MissingPort {
                loc: self.get_loc(),
                spec_msg: "data product specifiers".to_string(),
                port_msg: "product get port or product request port".to_string(),
            });
        }
        require(
            self.has_data_products(),
            "data product specifiers",
            &[ProductSend, TimeGet],
        )?;
        require(
            self.has_special_port(&ProductRequest),
            "product request specifier",
            &[ProductRecv],
        )?;
        Ok(())
    }

    /// Check that if there are any data products, then there are both containers
    /// and records
    fn check_data_products(&self) -> SemanticResult {
        match (self.record_map.len(), self.container_map.len()) {
            (0, 0) => Ok(()),
            (_, 0) => {
                let record = self.record_map.values().next().unwrap();
                Err(SemanticError::InvalidDataProducts {
                    loc: record.get_loc(),
                    msg: "component that specifies records must specify at least one container"
                        .to_string(),
                })
            }
            (0, _) => {
                let container = self.container_map.values().next().unwrap();
                Err(SemanticError::InvalidDataProducts {
                    loc: container.get_loc(),
                    msg: "component that specifies containers must specify at least one record"
                        .to_string(),
                })
            }
            _ => Ok(()),
        }
    }

    /// Construct the port matching list
    fn construct_port_matching_list(&self) -> SemanticResult<Vec<PortMatching>> {
        let mut list = Vec::new();
        for node in &self.spec_port_matching_list {
            list.push(self.construct_port_matching(node)?);
        }
        Ok(list)
    }

    /// Constructs a port matching from a specifier
    fn construct_port_matching(
        &self,
        node: &Arc<SpecPortMatching>,
    ) -> SemanticResult<PortMatching> {
        let loc = node.span();
        let name1 = &node.port1.data;
        let name2 = &node.port2.data;
        if name1 == name2 {
            return Err(SemanticError::InvalidPortMatching {
                loc,
                msg: format!("repeated name {}", name1),
            });
        }
        let get = |name: &str, span: Span| -> SemanticResult<GeneralPortInstance> {
            match self.port_map().get(name) {
                Some(PortInstance::General(pi)) => Ok(pi.clone()),
                Some(_) => Err(SemanticError::InvalidPortMatching {
                    loc: span,
                    msg: format!("{} is not a valid port instance for matching", name),
                }),
                None => Err(SemanticError::InvalidPortMatching {
                    loc: span,
                    msg: format!(
                        "{} is not a port instance of component {}",
                        name,
                        self.component_name()
                    ),
                }),
            }
        };
        let instance1 = get(name1, node.port1.span())?;
        let instance2 = get(name2, node.port2.span())?;
        let size1 = instance1.get_array_size();
        let size2 = instance2.get_array_size();
        if size1 != size2 {
            return Err(SemanticError::InvalidPortMatching {
                loc,
                msg: format!("mismatched port sizes ({} vs. {})", size1, size2),
            });
        }
        Ok(PortMatching {
            node: node.clone(),
            instance1,
            instance2,
        })
    }
}

/// Add an element to an id map, returning the updated map and the next default id
pub(crate) fn add_element_to_id_map<T>(
    map: &mut HashMap<i128, T>,
    id: i128,
    element: T,
    get_loc: impl Fn(&T) -> Span,
) -> SemanticResult<i128> {
    if let Some(prev) = map.get(&id) {
        return Err(SemanticError::DuplicateIdValue {
            value: display_id_value(id),
            loc: get_loc(&element),
            prev_loc: get_loc(prev),
        });
    }
    map.insert(id, element);
    Ok(id + 1)
}

/// Checks for duplicate names in dictionary.
pub(crate) fn check_dictionary_names<T>(
    map: &HashMap<i128, T>,
    kind: &str,
    get_name: impl Fn(&T) -> String,
    get_loc: impl Fn(&T) -> Span,
) -> SemanticResult {
    // Iterate in id order for deterministic diagnostics.
    let mut ids: Vec<&i128> = map.keys().collect();
    ids.sort();
    let mut seen: HashMap<String, Span> = HashMap::default();
    for id in ids {
        let value = &map[id];
        let name = get_name(value);
        let loc = get_loc(value);
        if let Some(prev_loc) = seen.insert(name.clone(), loc) {
            return Err(SemanticError::DuplicateDictionaryName {
                kind: kind.to_string(),
                name,
                loc,
                prev_loc,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Component;
    use crate::semantics::{PortInstance, Symbol, SymbolInterface, state_machine};
    use crate::{Analysis, add_state_enums, check_semantics};
    use fpp_ast::SpecialPortInstanceKind;
    use fpp_core::SourceFile;

    /// A component with a port matching over two general port arrays, one
    /// special port, and two state machine instances: one of an external state
    /// machine and one of an internal one.
    const SRC: &str = r#"
module Fw {
  port Cmd
  port CmdReg
  port CmdResponse
}

module M {

  port P

  state machine External

  state machine Internal {
    initial enter S
    state S
  }

  active component C {

    output port pOut: [2] P

    sync input port pIn: [2] P

    match pOut with pIn

    async input port pAsync: P

    command recv port cmdIn

    command reg port cmdRegOut

    command resp port cmdRespOut

    state machine instance smExternal: External

    state machine instance smInternal: Internal

  }

}
"#;

    /// Analyze [`SRC`] and hand the resolved component `C` to `f`. Panics if the
    /// input produced any diagnostic.
    fn with_component(f: impl FnOnce(&Component)) {
        let mut diagnostics = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
        fpp_core::run(&mut ctx, || {
            let source = SourceFile::new("component_test.fpp", SRC.to_string());
            let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let mut a = Analysis::new();
            add_state_enums(&mut ast);
            let _ = check_semantics(&mut a, vec![&ast]);
            let component = a
                .component_map
                .values()
                .find(|c| c.node.name.data == "C")
                .expect("component C was resolved");
            f(component);
        });
        let output = String::from_utf8(diagnostics).expect("diagnostics are UTF-8");
        assert_eq!(output, "", "expected no diagnostics");
    }

    /// The port maps a component exposes are its port interface's.
    #[test]
    fn port_maps_are_the_port_interface_maps() {
        with_component(|component| {
            assert_eq!(
                component.port_map().len(),
                component.port_interface.port_map.len()
            );
            assert!(component.port_map().contains_key("pOut"));
            assert_eq!(component.special_port_map().len(), 3);
            assert!(
                component
                    .special_port_map()
                    .contains_key(&SpecialPortInstanceKind::CommandRecv)
            );
        })
    }

    /// A port matching holds the two general port instances it matches, and
    /// shows as the specifier that produced it.
    #[test]
    fn port_matching_holds_general_port_instances() {
        with_component(|component| {
            let [matching] = &component.port_matching_list[..] else {
                panic!("one port matching was resolved");
            };
            assert_eq!(matching.instance1.get_unqualified_name(), "pOut");
            assert_eq!(matching.instance2.get_unqualified_name(), "pIn");
            // Both sides are arrays of the same size; that is what makes the
            // matching legal.
            assert_eq!(matching.instance1.get_array_size(), 2);
            assert_eq!(matching.instance2.get_array_size(), 2);
            assert_eq!(matching.to_string(), "match pOut with pIn");
            assert!(matching.matches(&PortInstance::General(matching.instance1.clone())));
            let async_port = component
                .port_map()
                .get("pAsync")
                .expect("the async input port was resolved");
            assert!(!matching.matches(async_port));
        })
    }

    /// Every state machine instance names a state machine, so its kind is
    /// always available.
    #[test]
    fn state_machine_instance_kinds() {
        with_component(|component| {
            let map = &component.state_machine_instance_map;
            let external = map.get("smExternal").expect("instance smExternal");
            let internal = map.get("smInternal").expect("instance smInternal");
            assert_eq!(external.get_sm_kind(), state_machine::Kind::External);
            assert_eq!(internal.get_sm_kind(), state_machine::Kind::Internal);
            assert!(matches!(external.symbol(), Symbol::StateMachine(_)));
            assert_eq!(external.symbol().name().data, "External");
            assert!(component.has_state_machine_instances());
            assert!(component.has_state_machine_instances_of_kind(&state_machine::Kind::External));
            assert!(component.has_state_machine_instances_of_kind(&state_machine::Kind::Internal));
        })
    }
}
