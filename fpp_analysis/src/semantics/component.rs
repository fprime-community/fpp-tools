use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{Format, PortInstance, PortInterface, Symbol, SymbolInterface, Type};
use fpp_ast::{
    AstNode, ComponentKind, DefComponent, InputPortKind, QueueFull, SpecCommand, SpecContainer,
    SpecEvent, SpecParam, SpecPortMatching, SpecRecord, SpecStateMachineInstance, SpecTlmChannel,
    SpecialPortInstanceKind,
};
use fpp_core::{Span, Spanned};
use rustc_hash::FxHashMap as HashMap;
use std::sync::Arc;

/// Display an id value as `(<dec> dec, <HEX> hex)`.
pub fn display_id_value(v: i128) -> String {
    format!("({} dec, {:X} hex)", v, v)
}

fn special_kind_key(kind: &SpecialPortInstanceKind) -> String {
    format!("{:?}", kind)
}

/// Render a special port kind in FPP style (lowercase, spaced).
fn special_kind_str(kind: &SpecialPortInstanceKind) -> &'static str {
    use SpecialPortInstanceKind::*;
    match kind {
        CommandRecv => "command recv",
        CommandReg => "command reg",
        CommandResp => "command resp",
        Event => "event",
        ParamGet => "param get",
        ParamSet => "param set",
        ProductGet => "product get",
        ProductRecv => "product recv",
        ProductRequest => "product request",
        ProductSend => "product send",
        Telemetry => "telemetry",
        TextEvent => "text event",
        TimeGet => "time get",
    }
}

/// Render a component kind in FPP style (lowercase).
pub fn component_kind_str(kind: &ComponentKind) -> &'static str {
    match kind {
        ComponentKind::Active => "active",
        ComponentKind::Passive => "passive",
        ComponentKind::Queued => "queued",
    }
}

/// A command.
#[derive(Debug, Clone)]
pub struct Command {
    pub loc: Span,
    pub name: String,
    /// The kind of a non-parameter command, or `None` for a param set/save command.
    pub kind: Option<CommandKind>,
}

#[derive(Debug, Clone)]
pub enum CommandKind {
    Async {
        priority: Option<i128>,
        queue_full: QueueFull,
    },
    Guarded,
    Sync,
}

impl Command {
    pub fn is_async(&self) -> bool {
        matches!(self.kind, Some(CommandKind::Async { .. }))
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
            if node.queue_full.is_some() {
                return Err(SemanticError::InvalidQueueFull { loc });
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
            InputPortKind::Async => CommandKind::Async {
                priority,
                queue_full: Analysis::get_queue_full(&node.queue_full),
            },
            InputPortKind::Guarded => CommandKind::Guarded,
            InputPortKind::Sync => CommandKind::Sync,
        };
        Ok(Command {
            loc,
            name: node.name.data.clone(),
            kind: Some(kind),
        })
    }
}

/// A telemetry channel.
#[derive(Debug, Clone)]
pub struct TlmChannel {
    pub loc: Span,
    pub name: String,
}

impl TlmChannel {
    /// Creates a telemetry channel from a telemetry channel specifier.
    pub fn from_spec(a: &Analysis, node: &SpecTlmChannel) -> SemanticResult<TlmChannel> {
        let loc = node.span();
        let channel_type = a.type_map.get(&node.type_name.node_id).unwrap().clone();
        if let Some(format) = &node.format {
            Format::new(format, vec![(channel_type, node.type_name.span())]);
        }
        compute_limits(a, &node.low)?;
        compute_limits(a, &node.high)?;
        a.check_displayable_type(
            node.type_name.node_id,
            node.type_name.span(),
            "type of telemetry channel is not displayable",
        )?;
        Ok(TlmChannel {
            loc,
            name: node.name.data.clone(),
        })
    }
}

/// Computes limits from AST limits.
fn compute_limits(_a: &Analysis, limits: &[fpp_ast::TlmChannelLimit]) -> SemanticResult {
    let mut seen: HashMap<String, Span> = HashMap::default();
    for limit in limits {
        let key = format!("{:?}", limit.kind);
        if let Some(prev_loc) = seen.insert(key, limit.value.span()) {
            return Err(SemanticError::DuplicateLimit {
                loc: limit.value.span(),
                prev_loc,
            });
        }
    }
    Ok(())
}

/// A data product record.
#[derive(Debug, Clone)]
pub struct Record {
    pub loc: Span,
    pub name: String,
}

impl Record {
    /// Creates a record from a record specifier.
    pub fn from_spec(a: &Analysis, node: &SpecRecord) -> SemanticResult<Record> {
        a.check_displayable_type(
            node.record_type.node_id,
            node.record_type.span(),
            "type of record is not displayable",
        )?;
        Ok(Record {
            loc: node.span(),
            name: node.name.data.clone(),
        })
    }
}

/// A data product container.
#[derive(Debug, Clone)]
pub struct Container {
    pub loc: Span,
    pub name: String,
}

impl Container {
    /// Creates a container from a container specifier.
    pub fn from_spec(a: &Analysis, node: &SpecContainer) -> SemanticResult<Container> {
        a.get_nonnegative_big_int_value_opt(&node.default_priority)?;
        Ok(Container {
            loc: node.span(),
            name: node.name.data.clone(),
        })
    }
}

/// A parameter.
#[derive(Debug, Clone)]
pub struct Param {
    pub loc: Span,
    pub name: String,
    pub set_opcode: i128,
    pub save_opcode: i128,
    pub is_external: bool,
}

impl Param {
    /// Create a parameter, returning it plus the updated default opcode.
    pub fn from_spec(
        a: &Analysis,
        node: &SpecParam,
        default_opcode: i128,
    ) -> SemanticResult<(Param, i128)> {
        let loc = node.span();
        // Check that the default value (if any) converts to the parameter type.
        // Resolve the finalized parameter type via its definition node, since the
        // type-name use node may hold an unfinalized type (e.g. array size unknown).
        if let Some(default) = &node.default
            && let (Some(default_ty), Some(param_ty)) = (
                a.type_map.get(&default.node_id).cloned(),
                a.type_map.get(&node.type_name.node_id).cloned(),
            )
        {
            let param_ty = match param_ty.def_node_id() {
                Some(def_node) => a.type_map.get(&def_node).cloned().unwrap_or(param_ty),
                None => param_ty,
            };
            if let Err(err) = Type::convert(&default_ty, &param_ty) {
                return Err(SemanticError::TypeConversion {
                    loc: default.span(),
                    msg: format!("default value cannot be converted to {}", param_ty),
                    err: Box::new(err),
                });
            }
        }
        a.check_displayable_type(
            node.type_name.node_id,
            node.type_name.span(),
            "type of parameter is not displayable",
        )?;
        let set_opcode_opt = a.get_nonnegative_big_int_value_opt(&node.set_opcode)?;
        let save_opcode_opt = a.get_nonnegative_big_int_value_opt(&node.save_opcode)?;
        let (set_opcode, default1) = compute_opcode(set_opcode_opt, default_opcode);
        let (save_opcode, default2) = compute_opcode(save_opcode_opt, default1);
        Ok((
            Param {
                loc,
                name: node.name.data.clone(),
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

/// An event.
#[derive(Debug, Clone)]
pub struct Event {
    pub loc: Span,
    pub name: String,
}

impl Event {
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
        Format::new(&node.format, types);
        if let Some(throttle) = &node.throttle {
            check_event_throttle(a, throttle, loc)?;
        }
        Ok(Event {
            loc,
            name: node.name.data.clone(),
        })
    }
}

fn check_event_throttle(
    a: &Analysis,
    throttle: &fpp_ast::EventThrottle,
    loc: Span,
) -> SemanticResult {
    let count = a.get_nonnegative_int_value(throttle.count.node_id, throttle.count.span())?;
    if count == 0 {
        return Err(SemanticError::InvalidEvent {
            loc,
            msg: "event throttle count must be greater than zero".to_string(),
        });
    }
    if let Some(every) = &throttle.every {
        check_throttle_interval(a, every, loc)?;
    }
    Ok(())
}

fn check_throttle_interval(a: &Analysis, every: &fpp_ast::Expr, loc: Span) -> SemanticResult {
    use crate::semantics::{AnonStructType, StructValue, Value};
    let u32_ty = Arc::new(Type::PrimitiveInt(fpp_ast::IntegerKind::U32));
    let mut members = HashMap::default();
    members.insert("seconds".to_string(), u32_ty.clone());
    members.insert("useconds".to_string(), u32_ty.clone());
    let interval_ty = Arc::new(Type::AnonStruct(AnonStructType { members }));

    let value = match a.value_map.get(&every.node_id) {
        Some(v) => v,
        None => return Ok(()),
    };
    let interval = match value.convert(&interval_ty) {
        Some(Value::Struct(StructValue { anon_struct, .. }))
        | Some(Value::AnonStruct(anon_struct)) => anon_struct,
        _ => {
            return Err(SemanticError::InvalidEvent {
                loc,
                msg: "event throttle interval must be a struct with seconds and useconds"
                    .to_string(),
            });
        }
    };

    check_interval_member(&interval, "seconds", u32::MAX as i128, loc)?;
    check_interval_member(&interval, "useconds", 999_999, loc)?;
    Ok(())
}

fn check_interval_member(
    interval: &crate::semantics::AnonStructValue,
    member: &str,
    max_value: i128,
    loc: Span,
) -> SemanticResult {
    use crate::semantics::{IntegerValue, PrimitiveIntegerValue, Value};
    let u32_ty = Arc::new(Type::PrimitiveInt(fpp_ast::IntegerKind::U32));
    let v = match interval
        .members
        .get(member)
        .and_then(|v| v.convert(&u32_ty))
    {
        Some(Value::PrimitiveInteger(PrimitiveIntegerValue { value, .. }))
        | Some(Value::Integer(IntegerValue(value))) => value,
        _ => {
            return Err(SemanticError::InvalidEvent {
                loc,
                msg: format!("event throttle interval is missing member {}", member),
            });
        }
    };
    if v < 0 || v > max_value {
        return Err(SemanticError::InvalidIntValue {
            loc,
            v: Some(v),
            msg: format!("{} must be in the range [0, {}]", member, max_value),
        });
    }
    Ok(())
}

/// A state machine instance.
#[derive(Debug, Clone)]
pub struct StateMachineInstance {
    pub loc: Span,
    pub name: String,
    pub symbol: Symbol,
}

impl StateMachineInstance {
    pub fn from_spec(
        a: &Analysis,
        node: &SpecStateMachineInstance,
    ) -> SemanticResult<Option<StateMachineInstance>> {
        let loc = node.span();
        let symbol = match a.use_def_map.get(&node.state_machine.id()) {
            Some(symbol @ Symbol::StateMachine(_)) => symbol.clone(),
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
            loc,
            name: node.name.data.clone(),
            symbol,
        }))
    }
}

/// An FPP component.
#[derive(Debug, Clone)]
pub struct Component {
    pub symbol: Symbol,
    /// The AST node defining the component.
    pub node: Arc<DefComponent>,
    pub loc: Span,
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

/// A resolved port matching between two general port instances.
#[derive(Debug, Clone)]
pub struct PortMatching {
    pub instance1: PortInstance,
    pub instance2: PortInstance,
    pub loc: Span,
}

impl PortMatching {
    /// Whether the given port instance participates in this matching.
    pub fn matches(&self, pi: &PortInstance) -> bool {
        self.instance1.get_node_id() == pi.get_node_id()
            || self.instance2.get_node_id() == pi.get_node_id()
    }
}

impl Component {
    pub fn new(symbol: Symbol, node: Arc<DefComponent>) -> Component {
        let loc = node.span();
        Component {
            symbol,
            node,
            loc,
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

    fn kind(&self) -> &ComponentKind {
        &self.node.kind
    }

    fn component_name(&self) -> &str {
        &self.node.name.data
    }

    /// Query whether the component has parameters
    pub fn has_parameters(&self) -> bool {
        !self.param_map.is_empty()
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
                loc: command.loc,
                prev_loc: prev.loc,
            });
        }
        self.command_map.insert(opcode, command);
        self.default_opcode = opcode + 1;
        Ok(())
    }

    /// Add a state machine instance
    pub fn add_state_machine_instance(&mut self, instance: StateMachineInstance) -> SemanticResult {
        if let Some(prev) = self.state_machine_instance_map.get(&instance.name) {
            return Err(SemanticError::DuplicateStateMachineInstance {
                name: instance.name.clone(),
                loc: instance.loc,
                prev_loc: prev.loc,
            });
        }
        self.state_machine_instance_map
            .insert(instance.name.clone(), instance);
        Ok(())
    }

    /// Add a data product container
    pub fn add_container(&mut self, id_opt: Option<i128>, container: Container) -> SemanticResult {
        let next = add_element_to_id_map(
            &mut self.container_map,
            id_opt.unwrap_or(self.default_container_id),
            container,
            |c| c.loc,
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
            |e| e.loc,
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
            |r| r.loc,
        )?;
        self.default_record_id = next;
        Ok(())
    }

    /// Add a telemetry channel
    pub fn add_tlm_channel(&mut self, id_opt: Option<i128>, channel: TlmChannel) -> SemanticResult {
        let name = channel.name.clone();
        let next = add_element_to_id_map(
            &mut self.tlm_channel_map,
            id_opt.unwrap_or(self.default_tlm_channel_id),
            channel.clone(),
            |t| t.loc,
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
        // Update the parameter map and the default parameter ID.
        let next = add_element_to_id_map(
            &mut self.param_map,
            id_opt.unwrap_or(self.default_param_id),
            param.clone(),
            |p| p.loc,
        )?;
        self.default_param_id = next;
        // Add the implicit set and save commands.
        let upper = param.name.to_uppercase();
        let set_command = Command {
            loc: param.loc,
            name: format!("{}_PRM_SET", upper),
            kind: None,
        };
        let save_command = Command {
            loc: param.loc,
            name: format!("{}_PRM_SAVE", upper),
            kind: None,
        };
        self.add_command(Some(param.set_opcode), set_command)?;
        self.add_command(Some(param.save_opcode), save_command)?;
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
        import_loc: Span,
    ) -> SemanticResult {
        let pi = self
            .port_interface
            .add_imported_interface(interface, import_loc)?;
        self.port_interface = pi;
        Ok(())
    }

    pub fn add_spec_port_matching(&mut self, node: Arc<SpecPortMatching>) {
        self.spec_port_matching_list.push(node);
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
        check_dictionary_names(&self.param_map, "parameter", |p| p.name.clone(), |p| p.loc)?;
        check_dictionary_names(&self.command_map, "command", |c| c.name.clone(), |c| c.loc)?;
        check_dictionary_names(&self.event_map, "event", |e| e.name.clone(), |e| e.loc)?;
        check_dictionary_names(
            &self.tlm_channel_map,
            "telemetry channel",
            |t| t.name.clone(),
            |t| t.loc,
        )?;
        check_dictionary_names(
            &self.container_map,
            "container",
            |c| c.name.clone(),
            |c| c.loc,
        )?;
        check_dictionary_names(&self.record_map, "record", |r| r.name.clone(), |r| r.loc)?;
        Ok(())
    }

    /// Checks that component has at least one async input port or async command
    fn check_async_input(&self) -> SemanticResult {
        // Component must have at least one async input port, async command, or SM instance.
        if self.check_no_async_input().is_err() {
            Ok(())
        } else {
            Err(SemanticError::MissingAsync {
                kind: component_kind_str(self.kind()).to_string(),
                loc: self.loc,
            })
        }
    }

    /// Checks that component has no async input ports
    fn check_no_async_input(&self) -> SemanticResult {
        for instance in self.port_interface.port_map.values() {
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
                    loc: command.loc,
                    import_locs: vec![],
                });
            }
        }
        if let Some(instance) = self.state_machine_instance_map.values().next() {
            return Err(SemanticError::PassiveStateMachine { loc: instance.loc });
        }
        Ok(())
    }

    fn has_special_port(&self, kind: SpecialPortInstanceKind) -> bool {
        self.port_interface
            .special_port_map
            .contains_key(&special_kind_key(&kind))
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
                    if !self.has_special_port(kind.clone()) {
                        return Err(SemanticError::MissingPort {
                            loc: self.loc,
                            spec_msg: spec_msg.to_string(),
                            port_msg: format!("{} port", special_kind_str(kind)),
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
            && !self.has_special_port(ProductGet)
            && !self.has_special_port(ProductRequest)
        {
            return Err(SemanticError::MissingPort {
                loc: self.loc,
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
            self.has_special_port(ProductRequest),
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
                    loc: record.loc,
                    msg: "component that specifies records must specify at least one container"
                        .to_string(),
                })
            }
            (0, _) => {
                let container = self.container_map.values().next().unwrap();
                Err(SemanticError::InvalidDataProducts {
                    loc: container.loc,
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
    fn construct_port_matching(&self, node: &SpecPortMatching) -> SemanticResult<PortMatching> {
        let loc = node.span();
        let name1 = &node.port1.data;
        let name2 = &node.port2.data;
        if name1 == name2 {
            return Err(SemanticError::InvalidPortMatching {
                loc,
                msg: format!("repeated name {}", name1),
            });
        }
        let get = |name: &str, span: Span| -> SemanticResult<PortInstance> {
            match self.port_interface.port_map.get(name) {
                Some(pi @ PortInstance::General { .. }) => Ok(pi.clone()),
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
            instance1,
            instance2,
            loc,
        })
    }
}

/// Add an element to an id map, returning the updated map and the next default id
fn add_element_to_id_map<T>(
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
fn check_dictionary_names<T>(
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
