use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    Command, CommandOpcode, Container, ContainerId, Event, EventId, GeneralPortInstance, Interface,
    NonParamKind, Param, ParamId, ParamKind, PortInstance, PortInterface, Record, RecordId,
    SpecialPortInstance, StateMachineInstance, Symbol, TlmChannel, TlmChannelId, state_machine,
};
use fpp_ast::{
    ComponentKind, DefComponent, Ident, InputPortKind, SpecPortMatching, SpecialPortInstanceKind,
};
use fpp_core::{Node, Span, Spanned};
use indexmap::IndexMap;
use rustc_hash::FxBuildHasher;
use std::sync::Arc;

/// An FPP component
#[derive(Debug, Clone)]
pub struct Component {
    /// The symbol of the component
    pub symbol: Symbol,
    /// The AST node defining the component
    pub node: Arc<DefComponent>,
    /// The port interface of the component
    pub port_interface: PortInterface,
    /// The map from command opcodes to commands
    pub command_map: IndexMap<CommandOpcode, Command, FxBuildHasher>,
    /// The next default opcode
    pub default_opcode: i128,
    /// The map from telemetry channel IDs to channels
    pub tlm_channel_map: IndexMap<TlmChannelId, TlmChannel, FxBuildHasher>,
    /// The map from telemetry channel names to channels
    pub tlm_channel_name_map: IndexMap<String, TlmChannel, FxBuildHasher>,
    /// The next default channel ID
    pub default_tlm_channel_id: i128,
    /// The map from event IDs to events
    pub event_map: IndexMap<EventId, Event, FxBuildHasher>,
    /// The next default event ID
    pub default_event_id: i128,
    /// The map from parameter IDs to parameters
    pub param_map: IndexMap<ParamId, Param, FxBuildHasher>,
    /// The list of port matching specifiers, most recent first
    pub spec_port_matching_list: Vec<Arc<SpecPortMatching>>,
    /// The map from state machine instance names to state machine instances
    pub state_machine_instance_map: IndexMap<String, StateMachineInstance, FxBuildHasher>,
    /// The list of port matching constraints
    pub port_matching_list: Vec<PortMatching>,
    /// The next default parameter ID
    pub default_param_id: i128,
    /// The map from container ids to containers
    pub container_map: IndexMap<ContainerId, Container, FxBuildHasher>,
    /// The next default container ID
    pub default_container_id: i128,
    /// The map from record ids to records
    pub record_map: IndexMap<RecordId, Record, FxBuildHasher>,
    /// The next default record ID
    pub default_record_id: i128,
}

impl Component {
    /// Creates a component with no members
    pub fn new(symbol: Symbol, node: Arc<DefComponent>) -> Component {
        Component {
            symbol,
            node,
            port_interface: PortInterface::new("component"),
            command_map: IndexMap::default(),
            default_opcode: 0,
            tlm_channel_map: IndexMap::default(),
            tlm_channel_name_map: IndexMap::default(),
            default_tlm_channel_id: 0,
            event_map: IndexMap::default(),
            default_event_id: 0,
            param_map: IndexMap::default(),
            spec_port_matching_list: vec![],
            state_machine_instance_map: IndexMap::default(),
            port_matching_list: vec![],
            default_param_id: 0,
            container_map: IndexMap::default(),
            default_container_id: 0,
            record_map: IndexMap::default(),
            default_record_id: 0,
        }
    }

    /// The map from port names to port instances
    pub fn port_map(&self) -> &IndexMap<String, PortInstance, FxBuildHasher> {
        &self.port_interface.port_map
    }

    /// The map from special port kinds to special port instances
    pub fn special_port_map(
        &self,
    ) -> &IndexMap<SpecialPortInstanceKind, SpecialPortInstance, FxBuildHasher> {
        &self.port_interface.special_port_map
    }

    /// Gets the kind of the component
    fn kind(&self) -> &ComponentKind {
        &self.node.kind
    }

    /// Gets the name of the component
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

    /// Query whether the component has state machine instances of
    /// the specified kind
    pub fn has_state_machine_instances_of_kind(&self, kind: &state_machine::Kind) -> bool {
        self.state_machine_instance_map
            .values()
            .any(|i| &i.get_sm_kind() == kind)
    }

    /// Gets the max identifier
    pub fn get_max_id(&self) -> i128 {
        fn max_in_map<T>(map: &IndexMap<i128, T, FxBuildHasher>) -> i128 {
            map.keys().copied().max().unwrap_or(-1)
        }
        [
            max_in_map(&self.command_map),
            max_in_map(&self.container_map),
            max_in_map(&self.event_map),
            max_in_map(&self.param_map),
            max_in_map(&self.record_map),
            max_in_map(&self.tlm_channel_map),
        ]
        .into_iter()
        .max()
        .unwrap_or(-1)
    }

    /// Gets a telemetry channel by name
    pub fn get_tlm_channel_by_name(&self, name: &Ident) -> SemanticResult<TlmChannel> {
        match self.tlm_channel_name_map.get(&name.data) {
            Some(tlm_channel) => Ok(tlm_channel.clone()),
            None => Err(SemanticError::InvalidTlmChannelName {
                loc: name.span(),
                channel_name: name.data.clone(),
                component_name: self.component_name().to_string(),
            }),
        }
    }

    /// Add a command
    pub fn add_command(
        &mut self,
        opcode_opt: Option<CommandOpcode>,
        command: Command,
    ) -> SemanticResult {
        let opcode = opcode_opt.unwrap_or(self.default_opcode);
        if let Some(prev_command) = self.command_map.get(&opcode) {
            return Err(SemanticError::DuplicateOpcodeValue {
                value: Analysis::display_id_value(opcode),
                loc: command.get_loc(),
                prev_loc: prev_command.get_loc(),
            });
        }
        let default_opcode =
            opcode
                .checked_add(1)
                .ok_or_else(|| SemanticError::InvalidIntValue {
                    loc: command.get_loc(),
                    v: None,
                    msg: format!("the default opcode after {} is out of range", opcode),
                })?;
        self.command_map.insert(opcode, command);
        self.default_opcode = default_opcode;
        Ok(())
    }

    /// Add a state machine instance
    pub fn add_state_machine_instance(&mut self, instance: StateMachineInstance) -> SemanticResult {
        let name = instance.get_name().to_string();
        if let Some(prev_instance) = self.state_machine_instance_map.get(&name) {
            return Err(SemanticError::DuplicateStateMachineInstance {
                name,
                loc: instance.get_loc(),
                prev_loc: prev_instance.get_loc(),
            });
        }
        self.state_machine_instance_map.insert(name, instance);
        Ok(())
    }

    /// Add a data product container
    pub fn add_container(
        &mut self,
        id_opt: Option<ContainerId>,
        container: Container,
    ) -> SemanticResult {
        self.default_container_id = Analysis::add_element_to_id_map(
            &mut self.container_map,
            id_opt.unwrap_or(self.default_container_id),
            container,
            Container::get_loc,
        )?;
        Ok(())
    }

    /// Add an event
    pub fn add_event(&mut self, id_opt: Option<EventId>, event: Event) -> SemanticResult {
        self.default_event_id = Analysis::add_element_to_id_map(
            &mut self.event_map,
            id_opt.unwrap_or(self.default_event_id),
            event,
            Event::get_loc,
        )?;
        Ok(())
    }

    /// Add a parameter
    pub fn add_param(&mut self, id_opt: Option<ParamId>, param: Param) -> SemanticResult {
        let set_opcode = param.set_opcode;
        let save_opcode = param.save_opcode;
        let set_command = Command::Param {
            node: param.node.clone(),
            kind: ParamKind::Set,
        };
        let save_command = Command::Param {
            node: param.node.clone(),
            kind: ParamKind::Save,
        };
        // Update the parameter map and the default parameter ID
        self.default_param_id = Analysis::add_element_to_id_map(
            &mut self.param_map,
            id_opt.unwrap_or(self.default_param_id),
            param,
            Param::get_loc,
        )?;
        // Add the implicit set and save commands
        self.add_command(Some(set_opcode), set_command)?;
        self.add_command(Some(save_opcode), save_command)?;
        Ok(())
    }

    /// Add a port instance
    pub fn add_port_instance(&mut self, instance: PortInstance) -> SemanticResult {
        self.port_interface = self.port_interface.add_port_instance(instance)?;
        Ok(())
    }

    /// Add the ports of an imported interface
    pub fn add_imported_interface(
        &mut self,
        interface: &Interface,
        import_node_id: Node,
    ) -> SemanticResult {
        self.port_interface = self
            .port_interface
            .add_imported_interface(interface, import_node_id)?;
        Ok(())
    }

    /// Add a data product record
    pub fn add_record(&mut self, id_opt: Option<RecordId>, record: Record) -> SemanticResult {
        self.default_record_id = Analysis::add_element_to_id_map(
            &mut self.record_map,
            id_opt.unwrap_or(self.default_record_id),
            record,
            Record::get_loc,
        )?;
        Ok(())
    }

    /// Add a telemetry channel
    pub fn add_tlm_channel(
        &mut self,
        id_opt: Option<TlmChannelId>,
        tlm_channel: TlmChannel,
    ) -> SemanticResult {
        let name = tlm_channel.get_name().to_string();
        self.default_tlm_channel_id = Analysis::add_element_to_id_map(
            &mut self.tlm_channel_map,
            id_opt.unwrap_or(self.default_tlm_channel_id),
            tlm_channel.clone(),
            TlmChannel::get_loc,
        )?;
        // Add the channel to the channel name map. If there is a duplicate name,
        // we will catch it later when we check all the dictionary elements.
        self.tlm_channel_name_map.insert(name, tlm_channel);
        Ok(())
    }

    /// Check that component provides ports required by dictionary
    /// and data product specifiers
    fn check_required_ports(&self) -> SemanticResult {
        use SpecialPortInstanceKind::*;
        let require_ports = |condition: bool,
                             spec_msg: &str,
                             port_kinds: &[SpecialPortInstanceKind]|
         -> SemanticResult {
            if condition {
                for port_kind in port_kinds {
                    if !self.special_port_map().contains_key(port_kind) {
                        return Err(SemanticError::MissingPort {
                            loc: self.get_loc(),
                            spec_msg: spec_msg.to_string(),
                            port_msg: format!("{} port", port_kind),
                        });
                    }
                }
            }
            Ok(())
        };
        let require_product_get_or_request = || -> SemanticResult {
            if self.has_data_products()
                && !self.special_port_map().contains_key(&ProductGet)
                && !self.special_port_map().contains_key(&ProductRequest)
            {
                Err(SemanticError::MissingPort {
                    loc: self.get_loc(),
                    spec_msg: "data product specifiers".to_string(),
                    port_msg: "product get port or product request port".to_string(),
                })
            } else {
                Ok(())
            }
        };
        require_ports(
            self.has_parameters(),
            "parameter specifiers",
            &[ParamGet, ParamSet, CommandRecv, CommandReg, CommandResp],
        )?;
        require_ports(
            self.has_commands(),
            "command specifiers",
            &[CommandRecv, CommandReg, CommandResp],
        )?;
        require_ports(
            self.has_events(),
            "event specifiers",
            &[Event, TextEvent, TimeGet],
        )?;
        require_ports(
            self.has_telemetry(),
            "telemetry specifiers",
            &[Telemetry, TimeGet],
        )?;
        if self.has_data_products() {
            require_product_get_or_request()?;
        }
        require_ports(
            self.has_data_products(),
            "data product specifiers",
            &[ProductSend, TimeGet],
        )?;
        require_ports(
            self.special_port_map().contains_key(&ProductRequest),
            "product request specifier",
            &[ProductRecv],
        )?;
        Ok(())
    }

    /// Check that if there are any data products, then there are both containers
    /// and records
    fn check_data_products(&self) -> SemanticResult {
        match (self.record_map.first(), self.container_map.first()) {
            (Some((_, record)), None) => Err(SemanticError::InvalidDataProducts {
                loc: record.get_loc(),
                msg: "component that specifies records must specify at least one container"
                    .to_string(),
            }),
            (None, Some((_, container))) => Err(SemanticError::InvalidDataProducts {
                loc: container.get_loc(),
                msg: "component that specifies containers must specify at least one record"
                    .to_string(),
            }),
            _ => Ok(()),
        }
    }

    /// Checks that component has at least one async input port or async command
    fn check_async_input(&self) -> SemanticResult {
        match self.check_no_async_input() {
            Err(_) => Ok(()),
            Ok(()) => Err(SemanticError::MissingAsync {
                kind: self.kind().to_string(),
                loc: self.get_loc(),
            }),
        }
    }

    /// Checks that component has no async input ports
    fn check_no_async_input(&self) -> SemanticResult {
        let check_port_instances = || -> SemanticResult {
            for instance in self.port_map().values() {
                let error = || SemanticError::PassiveAsync {
                    loc: instance.get_loc(),
                    import_locs: instance.get_import_locs(),
                };
                match instance {
                    PortInstance::General(general) => {
                        if general.is_async_input() {
                            return Err(error());
                        }
                    }
                    PortInstance::Special(special) => {
                        if matches!(special.node.input_kind, Some(InputPortKind::Async)) {
                            return Err(error());
                        }
                    }
                    PortInstance::Internal(_) => return Err(error()),
                    PortInstance::Topology(_) => {
                        unreachable!("topology port cannot exist in component")
                    }
                }
            }
            Ok(())
        };
        let check_commands = || -> SemanticResult {
            for command in self.command_map.values() {
                if let Command::NonParam {
                    kind: NonParamKind::Async { .. },
                    ..
                } = command
                {
                    return Err(SemanticError::PassiveAsync {
                        loc: command.get_loc(),
                        import_locs: vec![],
                    });
                }
            }
            Ok(())
        };
        let check_state_machines = || -> SemanticResult {
            match self.state_machine_instance_map.values().next() {
                Some(instance) => Err(SemanticError::PassiveStateMachine {
                    loc: instance.get_loc(),
                }),
                None => Ok(()),
            }
        };
        check_port_instances()?;
        check_commands()?;
        check_state_machines()?;
        Ok(())
    }

    /// Checks that there are no duplicate names in dictionaries
    fn check_no_duplicate_names(&self) -> SemanticResult {
        Analysis::check_dictionary_names(
            &self.param_map,
            "parameter",
            |p| p.get_name().to_string(),
            Param::get_loc,
        )?;
        Analysis::check_dictionary_names(
            &self.command_map,
            "command",
            Command::get_name,
            Command::get_loc,
        )?;
        Analysis::check_dictionary_names(
            &self.event_map,
            "event",
            |e| e.get_name().to_string(),
            Event::get_loc,
        )?;
        Analysis::check_dictionary_names(
            &self.tlm_channel_map,
            "telemetry channel",
            |t| t.get_name().to_string(),
            TlmChannel::get_loc,
        )?;
        Analysis::check_dictionary_names(
            &self.container_map,
            "container",
            |c| c.get_name().to_string(),
            Container::get_loc,
        )?;
        Analysis::check_dictionary_names(
            &self.record_map,
            "record",
            |r| r.get_name().to_string(),
            Record::get_loc,
        )?;
        Ok(())
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

    /// Complete a component definition
    pub fn complete(mut self) -> SemanticResult<Component> {
        self.port_matching_list = self.construct_port_matching_list()?;
        self.check_validity()?;
        Ok(self)
    }

    /// Construct the port matching list
    fn construct_port_matching_list(&self) -> SemanticResult<Vec<PortMatching>> {
        self.spec_port_matching_list
            .iter()
            .map(|node| self.construct_port_matching(node))
            .collect()
    }

    /// Constructs a port matching from a specifier
    fn construct_port_matching(
        &self,
        node: &Arc<SpecPortMatching>,
    ) -> SemanticResult<PortMatching> {
        let loc = node.span();
        let check_names = |name1: &str, name2: &str| -> SemanticResult {
            if name1 != name2 {
                Ok(())
            } else {
                Err(SemanticError::InvalidPortMatching {
                    loc,
                    msg: format!("repeated name {}", name1),
                })
            }
        };
        let get_instance = |node: &Ident| -> SemanticResult<GeneralPortInstance> {
            match self
                .port_interface
                .lookup_port_instance(node, self.component_name())?
            {
                PortInstance::General(general) => Ok(general),
                _ => Err(SemanticError::InvalidPortMatching {
                    loc: node.span(),
                    msg: format!("{} is not a valid port instance for matching", node.data),
                }),
            }
        };
        let check_sizes =
            |instance1: &GeneralPortInstance, instance2: &GeneralPortInstance| -> SemanticResult {
                let size1 = instance1.get_array_size();
                let size2 = instance2.get_array_size();
                if size1 == size2 {
                    Ok(())
                } else {
                    Err(SemanticError::InvalidPortMatching {
                        loc,
                        msg: format!("mismatched port sizes ({} vs. {})", size1, size2),
                    })
                }
            };
        let port1 = &node.port1;
        let port2 = &node.port2;
        check_names(&port1.data, &port2.data)?;
        let instance1 = get_instance(port1)?;
        let instance2 = get_instance(port2)?;
        check_sizes(&instance1, &instance2)?;
        Ok(PortMatching {
            node: node.clone(),
            instance1,
            instance2,
        })
    }
}

/// A port matching
#[derive(Debug, Clone)]
pub struct PortMatching {
    pub node: Arc<SpecPortMatching>,
    pub instance1: GeneralPortInstance,
    pub instance2: GeneralPortInstance,
}

impl PortMatching {
    /// Gets the location of a port matching
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Whether the given port instance is one of the two ports of this
    /// matching. Rust-only; Connection::is_match_constrained uses it.
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

#[cfg(test)]
mod tests {
    use super::Component;
    use crate::semantics::{PortInstance, Symbol, SymbolInterface, state_machine};
    use crate::{Analysis, add_state_enums, check_semantics, resolve_templates};
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
            let _ = resolve_templates(&mut a, std::slice::from_mut(&mut ast));
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
