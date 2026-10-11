//! A class for resolving connection patterns

use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    Component, ComponentInstance, Connection, ConnectionPattern, Direction, Endpoint,
    InterfaceInstance, PortInstance, PortInstanceIdentifier,
};
use fpp_ast::{ConnectionPatternKind, SpecialPortInstanceKind};
use fpp_core::Span;

/// A named connection
pub type NamedConnection = (String, Connection);

/// A component instance used at a location
type InstanceUse = (ComponentInstance, Span);

/// A class for resolving connection patterns
trait PatternResolver {
    /// The type of a source
    type Source;

    /// The type of the target
    type Target;

    /// The enclosing analysis
    fn a(&self) -> &Analysis;

    /// The connection pattern to resolve
    fn pattern(&self) -> &ConnectionPattern;

    /// The component instances in scope
    fn instances(&self) -> &[ComponentInstance];

    /// Resolve the source
    fn resolve_source(&self) -> SemanticResult<Self::Source>;

    /// Resolve a target
    fn resolve_target(&self, target_use: &InstanceUse) -> SemanticResult<Self::Target>;

    /// Generate the connections for a source and target
    fn get_connections_for_target(
        &self,
        source: &Self::Source,
        target: &Self::Target,
    ) -> Vec<NamedConnection>;

    /// Resolve the pattern to a list of connections
    fn resolve(&self) -> SemanticResult<Vec<NamedConnection>> {
        let source = self.resolve_source()?;
        let targets = self.resolve_targets()?;
        Ok(targets
            .iter()
            .flat_map(|target| self.get_connections_for_target(&source, target))
            .collect())
    }

    /// Gets the specified general port from a component instance
    fn get_general_port(
        &self,
        ci_use: &InstanceUse,
        kind: &str,
        direction: Direction,
        port_type_name: &str,
    ) -> SemanticResult<PortInstanceIdentifier> {
        let is_general_port =
            |pi: &&PortInstance| self.a().is_general_port(pi, direction, port_type_name);
        let (ci, loc) = ci_use;
        let ports: Vec<PortInstance> = component(self.a(), ci)
            .port_map()
            .values()
            .filter(is_general_port)
            .cloned()
            .collect();
        let pii = resolve_to_single_port(
            ports,
            &format!("{} {}", kind, Direction::show(&Some(direction))),
            *loc,
            ci.get_unqualified_name(),
        )?;
        Ok(PortInstanceIdentifier {
            interface_instance: InterfaceInstance::from_component_instance(ci.clone()),
            port_instance: pii,
        })
    }

    /// Resolve the targets: the explicit targets if there are any, otherwise
    /// the instances in scope
    fn resolve_targets(&self) -> SemanticResult<Vec<Self::Target>> {
        match self.pattern().targets.len() {
            0 => Ok(self.resolve_implicit_targets()),
            _ => self.resolve_explicit_targets(),
        }
    }

    /// Resolve the instances in scope that have the ports a target needs
    fn resolve_implicit_targets(&self) -> Vec<Self::Target> {
        let loc = self.pattern().get_loc();
        self.instances()
            .iter()
            .filter_map(|i| self.resolve_target(&(i.clone(), loc)).ok())
            .collect()
    }

    /// Resolve the explicit targets, each of which must have the ports a target
    /// needs
    fn resolve_explicit_targets(&self) -> SemanticResult<Vec<Self::Target>> {
        self.pattern()
            .targets
            .iter()
            .map(|target| self.resolve_target(target))
            .collect()
    }
}

/// Resolve a pattern
pub fn resolve(
    a: &Analysis,
    pattern: &ConnectionPattern,
    instances: &[ComponentInstance],
) -> SemanticResult<Vec<NamedConnection>> {
    match pattern.kind() {
        ConnectionPatternKind::Command => Command {
            a,
            pattern,
            instances,
        }
        .resolve(),
        ConnectionPatternKind::Event => {
            FromSpecialTargetPort::event(a, pattern, instances).resolve()
        }
        ConnectionPatternKind::Health => Health {
            a,
            pattern,
            instances,
        }
        .resolve(),
        ConnectionPatternKind::Param => Param {
            a,
            pattern,
            instances,
        }
        .resolve(),
        ConnectionPatternKind::Telemetry => {
            FromSpecialTargetPort::telemetry(a, pattern, instances).resolve()
        }
        ConnectionPatternKind::TextEvent => {
            FromSpecialTargetPort::text_event(a, pattern, instances).resolve()
        }
        ConnectionPatternKind::Time => FromSpecialTargetPort::time(a, pattern, instances).resolve(),
    }
}

fn connect(
    loc: Span,
    from_pii: PortInstanceIdentifier,
    to_pii: PortInstanceIdentifier,
) -> Connection {
    let from = Endpoint::new(loc, from_pii);
    let to = Endpoint::new(loc, to_pii);
    Connection::new(from, to)
}

fn missing_port<T>(loc: Span, kind: &str, instance_name: &str) -> SemanticResult<T> {
    Err(SemanticError::InvalidPattern {
        loc,
        msg: format!("instance {} has no {} port", instance_name, kind),
    })
}

/// The component of a component instance. An instance is entered into the
/// component instance map only once its component is in the component map.
fn component<'a>(a: &'a Analysis, ci: &ComponentInstance) -> &'a Component {
    ci.get_component(a)
        .expect("the component of a component instance is in the component map")
}

fn get_special_port(
    a: &Analysis,
    ci_use: &InstanceUse,
    kind: SpecialPortInstanceKind,
) -> SemanticResult<PortInstanceIdentifier> {
    let (ci, loc) = ci_use;
    match component(a, ci).special_port_map().get(&kind) {
        Some(pi) => Ok(PortInstanceIdentifier {
            interface_instance: InterfaceInstance::from_component_instance(ci.clone()),
            port_instance: PortInstance::Special(pi.clone()),
        }),
        None => missing_port(*loc, &kind.to_string(), ci.get_unqualified_name()),
    }
}

fn resolve_to_single_port(
    mut ports: Vec<PortInstance>,
    kind: &str,
    loc: Span,
    instance_name: &str,
) -> SemanticResult<PortInstance> {
    match ports.len() {
        1 => Ok(ports.pop().expect("there is one port")),
        0 => missing_port(loc, kind, instance_name),
        _ => {
            let mut port_names: Vec<&str> =
                ports.iter().map(|p| p.get_unqualified_name()).collect();
            port_names.sort();
            Err(SemanticError::InvalidPattern {
                loc,
                msg: format!(
                    "ambiguous pattern: instance {} has {} ports {}",
                    instance_name,
                    kind,
                    port_names.join(", ")
                ),
            })
        }
    }
}

/// Resolve a command pattern
struct Command<'a> {
    a: &'a Analysis,
    pattern: &'a ConnectionPattern,
    instances: &'a [ComponentInstance],
}

/// The source ports of a command pattern
struct CommandSource {
    cmd_reg_in: PortInstanceIdentifier,
    cmd_out: PortInstanceIdentifier,
    cmd_response_in: PortInstanceIdentifier,
}

/// The target ports of a command pattern
struct CommandTarget {
    cmd_reg_out: PortInstanceIdentifier,
    cmd_in: PortInstanceIdentifier,
    cmd_response_out: PortInstanceIdentifier,
}

impl Command<'_> {
    fn get_cmd_reg_in(&self) -> SemanticResult<PortInstanceIdentifier> {
        self.get_general_port(
            &self.pattern.source,
            "command reg",
            Direction::Input,
            "Fw.CmdReg",
        )
    }

    fn get_cmd_out(&self) -> SemanticResult<PortInstanceIdentifier> {
        self.get_general_port(
            &self.pattern.source,
            "command send",
            Direction::Output,
            "Fw.Cmd",
        )
    }

    fn get_cmd_response_in(&self) -> SemanticResult<PortInstanceIdentifier> {
        self.get_general_port(
            &self.pattern.source,
            "command resp",
            Direction::Input,
            "Fw.CmdResponse",
        )
    }

    fn get_cmd_reg_out(&self, target_use: &InstanceUse) -> SemanticResult<PortInstanceIdentifier> {
        get_special_port(self.a, target_use, SpecialPortInstanceKind::CommandReg)
    }

    fn get_cmd_in(&self, target_use: &InstanceUse) -> SemanticResult<PortInstanceIdentifier> {
        get_special_port(self.a, target_use, SpecialPortInstanceKind::CommandRecv)
    }

    fn get_cmd_response_out(
        &self,
        target_use: &InstanceUse,
    ) -> SemanticResult<PortInstanceIdentifier> {
        get_special_port(self.a, target_use, SpecialPortInstanceKind::CommandResp)
    }
}

impl PatternResolver for Command<'_> {
    type Source = CommandSource;
    type Target = CommandTarget;

    fn a(&self) -> &Analysis {
        self.a
    }

    fn pattern(&self) -> &ConnectionPattern {
        self.pattern
    }

    fn instances(&self) -> &[ComponentInstance] {
        self.instances
    }

    fn resolve_source(&self) -> SemanticResult<CommandSource> {
        Ok(CommandSource {
            cmd_reg_in: self.get_cmd_reg_in()?,
            cmd_out: self.get_cmd_out()?,
            cmd_response_in: self.get_cmd_response_in()?,
        })
    }

    fn resolve_target(&self, target_use: &InstanceUse) -> SemanticResult<CommandTarget> {
        Ok(CommandTarget {
            cmd_reg_out: self.get_cmd_reg_out(target_use)?,
            cmd_in: self.get_cmd_in(target_use)?,
            cmd_response_out: self.get_cmd_response_out(target_use)?,
        })
    }

    fn get_connections_for_target(
        &self,
        source: &CommandSource,
        target: &CommandTarget,
    ) -> Vec<NamedConnection> {
        let loc = self.pattern.get_loc();
        vec![
            (
                "CommandRegistration".to_string(),
                connect(loc, target.cmd_reg_out.clone(), source.cmd_reg_in.clone()),
            ),
            (
                "Command".to_string(),
                connect(loc, source.cmd_out.clone(), target.cmd_in.clone()),
            ),
            (
                "CommandResponse".to_string(),
                connect(
                    loc,
                    target.cmd_response_out.clone(),
                    source.cmd_response_in.clone(),
                ),
            ),
        ]
    }
}

/// Resolve a pattern involving connections from a single special
/// target port
struct FromSpecialTargetPort<'a> {
    a: &'a Analysis,
    pattern: &'a ConnectionPattern,
    instances: &'a [ComponentInstance],
    kind: SpecialPortInstanceKind,
    port_type_name: &'static str,
    graph_name: &'static str,
}

impl<'a> FromSpecialTargetPort<'a> {
    fn event(
        a: &'a Analysis,
        pattern: &'a ConnectionPattern,
        instances: &'a [ComponentInstance],
    ) -> Self {
        FromSpecialTargetPort {
            a,
            pattern,
            instances,
            kind: SpecialPortInstanceKind::Event,
            port_type_name: "Fw.Log",
            graph_name: "Events",
        }
    }

    fn telemetry(
        a: &'a Analysis,
        pattern: &'a ConnectionPattern,
        instances: &'a [ComponentInstance],
    ) -> Self {
        FromSpecialTargetPort {
            a,
            pattern,
            instances,
            kind: SpecialPortInstanceKind::Telemetry,
            port_type_name: "Fw.Tlm",
            graph_name: "Telemetry",
        }
    }

    fn text_event(
        a: &'a Analysis,
        pattern: &'a ConnectionPattern,
        instances: &'a [ComponentInstance],
    ) -> Self {
        FromSpecialTargetPort {
            a,
            pattern,
            instances,
            kind: SpecialPortInstanceKind::TextEvent,
            port_type_name: "Fw.LogText",
            graph_name: "TextEvents",
        }
    }

    fn time(
        a: &'a Analysis,
        pattern: &'a ConnectionPattern,
        instances: &'a [ComponentInstance],
    ) -> Self {
        FromSpecialTargetPort {
            a,
            pattern,
            instances,
            kind: SpecialPortInstanceKind::TimeGet,
            port_type_name: "Fw.Time",
            graph_name: "Time",
        }
    }
}

impl PatternResolver for FromSpecialTargetPort<'_> {
    type Source = PortInstanceIdentifier;
    type Target = PortInstanceIdentifier;

    fn a(&self) -> &Analysis {
        self.a
    }

    fn pattern(&self) -> &ConnectionPattern {
        self.pattern
    }

    fn instances(&self) -> &[ComponentInstance] {
        self.instances
    }

    fn resolve_source(&self) -> SemanticResult<PortInstanceIdentifier> {
        self.get_general_port(
            &self.pattern.source,
            &self.kind.to_string(),
            Direction::Input,
            self.port_type_name,
        )
    }

    fn resolve_target(&self, target_use: &InstanceUse) -> SemanticResult<PortInstanceIdentifier> {
        get_special_port(self.a, target_use, self.kind.clone())
    }

    fn get_connections_for_target(
        &self,
        source: &PortInstanceIdentifier,
        target: &PortInstanceIdentifier,
    ) -> Vec<NamedConnection> {
        vec![(
            self.graph_name.to_string(),
            connect(self.pattern.get_loc(), target.clone(), source.clone()),
        )]
    }
}

/// Resolve a health pattern
struct Health<'a> {
    a: &'a Analysis,
    pattern: &'a ConnectionPattern,
    instances: &'a [ComponentInstance],
}

/// The ping ports of a health pattern source or target
struct PingPorts {
    ping_in: PortInstanceIdentifier,
    ping_out: PortInstanceIdentifier,
}

impl Health<'_> {
    fn get_ping_port(
        &self,
        ci_use: &InstanceUse,
        direction: Direction,
    ) -> SemanticResult<PortInstanceIdentifier> {
        self.get_general_port(ci_use, "ping", direction, "Svc.Ping")
    }

    fn get_ping_ports(&self, ci_use: &InstanceUse) -> SemanticResult<PingPorts> {
        Ok(PingPorts {
            ping_in: self.get_ping_port(ci_use, Direction::Input)?,
            ping_out: self.get_ping_port(ci_use, Direction::Output)?,
        })
    }
}

impl PatternResolver for Health<'_> {
    type Source = PingPorts;
    type Target = PingPorts;

    fn a(&self) -> &Analysis {
        self.a
    }

    fn pattern(&self) -> &ConnectionPattern {
        self.pattern
    }

    fn instances(&self) -> &[ComponentInstance] {
        self.instances
    }

    fn resolve_source(&self) -> SemanticResult<PingPorts> {
        self.get_ping_ports(&self.pattern.source)
    }

    fn resolve_target(&self, target_use: &InstanceUse) -> SemanticResult<PingPorts> {
        self.get_ping_ports(target_use)
    }

    fn get_connections_for_target(
        &self,
        source: &PingPorts,
        target: &PingPorts,
    ) -> Vec<NamedConnection> {
        // Health component does not ping itself
        if source.ping_out.interface_instance != target.ping_in.interface_instance {
            let loc = self.pattern.get_loc();
            vec![
                connect(loc, source.ping_out.clone(), target.ping_in.clone()),
                connect(loc, target.ping_out.clone(), source.ping_in.clone()),
            ]
            .into_iter()
            .map(|c| ("Health".to_string(), c))
            .collect()
        } else {
            vec![]
        }
    }
}

/// Resolve a param pattern
struct Param<'a> {
    a: &'a Analysis,
    pattern: &'a ConnectionPattern,
    instances: &'a [ComponentInstance],
}

/// The source ports of a param pattern
struct ParamSource {
    prm_get_in: PortInstanceIdentifier,
    prm_set_in: PortInstanceIdentifier,
}

/// The target ports of a param pattern
struct ParamTarget {
    prm_get_out: PortInstanceIdentifier,
    prm_set_out: PortInstanceIdentifier,
}

impl Param<'_> {
    fn get_prm_get_in(&self) -> SemanticResult<PortInstanceIdentifier> {
        self.get_general_port(
            &self.pattern.source,
            "param get",
            Direction::Input,
            "Fw.PrmGet",
        )
    }

    fn get_prm_set_in(&self) -> SemanticResult<PortInstanceIdentifier> {
        self.get_general_port(
            &self.pattern.source,
            "param set",
            Direction::Input,
            "Fw.PrmSet",
        )
    }

    fn get_prm_get_out(&self, target_use: &InstanceUse) -> SemanticResult<PortInstanceIdentifier> {
        get_special_port(self.a, target_use, SpecialPortInstanceKind::ParamGet)
    }

    fn get_prm_set_out(&self, target_use: &InstanceUse) -> SemanticResult<PortInstanceIdentifier> {
        get_special_port(self.a, target_use, SpecialPortInstanceKind::ParamSet)
    }
}

impl PatternResolver for Param<'_> {
    type Source = ParamSource;
    type Target = ParamTarget;

    fn a(&self) -> &Analysis {
        self.a
    }

    fn pattern(&self) -> &ConnectionPattern {
        self.pattern
    }

    fn instances(&self) -> &[ComponentInstance] {
        self.instances
    }

    fn resolve_source(&self) -> SemanticResult<ParamSource> {
        Ok(ParamSource {
            prm_get_in: self.get_prm_get_in()?,
            prm_set_in: self.get_prm_set_in()?,
        })
    }

    fn resolve_target(&self, target_use: &InstanceUse) -> SemanticResult<ParamTarget> {
        Ok(ParamTarget {
            prm_get_out: self.get_prm_get_out(target_use)?,
            prm_set_out: self.get_prm_set_out(target_use)?,
        })
    }

    fn get_connections_for_target(
        &self,
        source: &ParamSource,
        target: &ParamTarget,
    ) -> Vec<NamedConnection> {
        let loc = self.pattern.get_loc();
        vec![
            connect(loc, target.prm_get_out.clone(), source.prm_get_in.clone()),
            connect(loc, target.prm_set_out.clone(), source.prm_set_in.clone()),
        ]
        .into_iter()
        .map(|c| ("Parameters".to_string(), c))
        .collect()
    }
}
