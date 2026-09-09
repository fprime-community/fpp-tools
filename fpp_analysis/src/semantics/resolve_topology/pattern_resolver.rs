// Resolve connection patterns

use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    ComponentInstance, Connection, ConnectionPattern, Direction, Endpoint, InterfaceInstance,
    PortInstance, PortInstanceIdentifier,
};
use fpp_ast::{ConnectionPatternKind, SpecialPortInstanceKind};
use fpp_core::Span;

/// The human-readable name of a special port kind.
fn special_kind_str(kind: &SpecialPortInstanceKind) -> &'static str {
    match kind {
        SpecialPortInstanceKind::CommandRecv => "command recv",
        SpecialPortInstanceKind::CommandReg => "command reg",
        SpecialPortInstanceKind::CommandResp => "command resp",
        SpecialPortInstanceKind::Event => "event",
        SpecialPortInstanceKind::ParamGet => "param get",
        SpecialPortInstanceKind::ParamSet => "param set",
        SpecialPortInstanceKind::ProductGet => "product get",
        SpecialPortInstanceKind::ProductRecv => "product recv",
        SpecialPortInstanceKind::ProductRequest => "product request",
        SpecialPortInstanceKind::ProductSend => "product send",
        SpecialPortInstanceKind::Telemetry => "telemetry",
        SpecialPortInstanceKind::TextEvent => "text event",
        SpecialPortInstanceKind::TimeGet => "time get",
    }
}

fn dir_str(d: Direction) -> &'static str {
    match d {
        Direction::Input => "input",
        Direction::Output => "output",
    }
}

fn missing_port<T>(loc: Span, kind: &str, instance_name: &str) -> SemanticResult<T> {
    Err(SemanticError::InvalidPattern {
        loc,
        msg: format!("instance {} has no {} port", instance_name, kind),
    })
}

fn connect(loc: Span, from: PortInstanceIdentifier, to: PortInstanceIdentifier) -> Connection {
    Connection::new(Endpoint::new(loc, from), Endpoint::new(loc, to))
}

fn get_component<'a>(
    a: &'a Analysis,
    ci: &ComponentInstance,
) -> Option<&'a crate::semantics::Component> {
    a.component_map.get(&ci.component_symbol)
}

fn resolve_to_single_port(
    mut ports: Vec<PortInstance>,
    kind: &str,
    loc: Span,
    instance_name: &str,
) -> SemanticResult<PortInstance> {
    match ports.len() {
        1 => Ok(ports.pop().unwrap()),
        0 => missing_port(loc, kind, instance_name),
        _ => {
            let mut names: Vec<String> = ports
                .iter()
                .map(|p| p.get_unqualified_name().to_string())
                .collect();
            names.sort();
            Err(SemanticError::InvalidPattern {
                loc,
                msg: format!(
                    "ambiguous pattern: instance {} has {} ports {}",
                    instance_name,
                    kind,
                    names.join(", ")
                ),
            })
        }
    }
}

/// Gets the specified general port from a component instance
fn get_general_port(
    a: &Analysis,
    ci_use: &(ComponentInstance, Span),
    kind: &str,
    direction: Direction,
    port_type_name: &str,
) -> SemanticResult<PortInstanceIdentifier> {
    let (ci, loc) = ci_use;
    let Some(comp) = get_component(a, ci) else {
        return missing_port(*loc, &format!("{} {}", kind, dir_str(direction)), &ci.name);
    };
    let ports: Vec<PortInstance> = comp
        .port_interface
        .port_map
        .values()
        .filter(|pi| a.is_general_port(pi, direction, port_type_name))
        .cloned()
        .collect();
    let pi = resolve_to_single_port(
        ports,
        &format!("{} {}", kind, dir_str(direction)),
        *loc,
        &ci.name,
    )?;
    Ok(PortInstanceIdentifier {
        interface_instance: InterfaceInstance::from_component_instance(ci.clone()),
        port_instance: pi,
    })
}

fn get_special_port(
    a: &Analysis,
    ci_use: &(ComponentInstance, Span),
    kind: SpecialPortInstanceKind,
) -> SemanticResult<PortInstanceIdentifier> {
    let (ci, loc) = ci_use;
    let Some(comp) = get_component(a, ci) else {
        return missing_port(*loc, special_kind_str(&kind), &ci.name);
    };
    let key = format!("{:?}", kind);
    match comp.port_interface.special_port_map.get(&key) {
        Some(pi) => Ok(PortInstanceIdentifier {
            interface_instance: InterfaceInstance::from_component_instance(ci.clone()),
            port_instance: pi.clone(),
        }),
        None => missing_port(*loc, special_kind_str(&kind), &ci.name),
    }
}

/// Resolve targets: explicit list, or all instances if none are specified.
fn resolve_targets<'a>(
    pattern: &'a ConnectionPattern,
    instances: &'a [ComponentInstance],
) -> Vec<(ComponentInstance, Span)> {
    if pattern.targets.is_empty() {
        instances
            .iter()
            .map(|ci| (ci.clone(), pattern.loc))
            .collect()
    } else {
        pattern.targets.clone()
    }
}

/// Resolve a pattern to a list of named connections.
pub fn resolve(
    a: &Analysis,
    pattern: &ConnectionPattern,
    instances: &[ComponentInstance],
) -> SemanticResult<Vec<(String, Connection)>> {
    match pattern.kind {
        ConnectionPatternKind::Command => resolve_command(a, pattern, instances),
        ConnectionPatternKind::Event => resolve_from_special(
            a,
            pattern,
            instances,
            SpecialPortInstanceKind::Event,
            "Fw.Log",
            "Events",
        ),
        ConnectionPatternKind::Telemetry => resolve_from_special(
            a,
            pattern,
            instances,
            SpecialPortInstanceKind::Telemetry,
            "Fw.Tlm",
            "Telemetry",
        ),
        ConnectionPatternKind::TextEvent => resolve_from_special(
            a,
            pattern,
            instances,
            SpecialPortInstanceKind::TextEvent,
            "Fw.LogText",
            "TextEvents",
        ),
        ConnectionPatternKind::Time => resolve_from_special(
            a,
            pattern,
            instances,
            SpecialPortInstanceKind::TimeGet,
            "Fw.Time",
            "Time",
        ),
        ConnectionPatternKind::Health => resolve_health(a, pattern, instances),
        ConnectionPatternKind::Param => resolve_param(a, pattern, instances),
    }
}

/// Iterate over targets, collecting connections. Explicit targets propagate
/// errors; implicit targets that fail resolution are skipped.
fn for_targets(
    pattern: &ConnectionPattern,
    instances: &[ComponentInstance],
    resolve_one: impl Fn(&(ComponentInstance, Span)) -> SemanticResult<Vec<(String, Connection)>>,
) -> SemanticResult<Vec<(String, Connection)>> {
    let explicit = !pattern.targets.is_empty();
    let mut result = Vec::new();
    for target_use in resolve_targets(pattern, instances) {
        match resolve_one(&target_use) {
            Ok(mut cs) => result.append(&mut cs),
            Err(e) => {
                if explicit {
                    return Err(e);
                }
            }
        }
    }
    Ok(result)
}

/// Resolve a command pattern
fn resolve_command(
    a: &Analysis,
    pattern: &ConnectionPattern,
    instances: &[ComponentInstance],
) -> SemanticResult<Vec<(String, Connection)>> {
    let loc = pattern.loc;
    let cmd_reg_in = get_general_port(
        a,
        &pattern.source,
        "command reg",
        Direction::Input,
        "Fw.CmdReg",
    )?;
    let cmd_out = get_general_port(
        a,
        &pattern.source,
        "command send",
        Direction::Output,
        "Fw.Cmd",
    )?;
    let cmd_response_in = get_general_port(
        a,
        &pattern.source,
        "command resp",
        Direction::Input,
        "Fw.CmdResponse",
    )?;

    for_targets(pattern, instances, |target| {
        let cmd_reg_out = get_special_port(a, target, SpecialPortInstanceKind::CommandReg)?;
        let cmd_in = get_special_port(a, target, SpecialPortInstanceKind::CommandRecv)?;
        let cmd_response_out = get_special_port(a, target, SpecialPortInstanceKind::CommandResp)?;
        Ok(vec![
            (
                "CommandRegistration".to_string(),
                connect(loc, cmd_reg_out, cmd_reg_in.clone()),
            ),
            ("Command".to_string(), connect(loc, cmd_out.clone(), cmd_in)),
            (
                "CommandResponse".to_string(),
                connect(loc, cmd_response_out, cmd_response_in.clone()),
            ),
        ])
    })
}

/// Resolve a pattern involving connections from a single special target port
fn resolve_from_special(
    a: &Analysis,
    pattern: &ConnectionPattern,
    instances: &[ComponentInstance],
    kind: SpecialPortInstanceKind,
    port_type_name: &str,
    graph_name: &str,
) -> SemanticResult<Vec<(String, Connection)>> {
    let loc = pattern.loc;
    let source = get_general_port(
        a,
        &pattern.source,
        special_kind_str(&kind),
        Direction::Input,
        port_type_name,
    )?;
    for_targets(pattern, instances, |target| {
        let target_port = get_special_port(a, target, kind.clone())?;
        Ok(vec![(
            graph_name.to_string(),
            connect(loc, target_port, source.clone()),
        )])
    })
}

fn get_ping_ports(
    a: &Analysis,
    ci_use: &(ComponentInstance, Span),
) -> SemanticResult<(PortInstanceIdentifier, PortInstanceIdentifier)> {
    let ping_in = get_general_port(a, ci_use, "ping", Direction::Input, "Svc.Ping")?;
    let ping_out = get_general_port(a, ci_use, "ping", Direction::Output, "Svc.Ping")?;
    Ok((ping_in, ping_out))
}

/// Resolve a health pattern
fn resolve_health(
    a: &Analysis,
    pattern: &ConnectionPattern,
    instances: &[ComponentInstance],
) -> SemanticResult<Vec<(String, Connection)>> {
    let loc = pattern.loc;
    let (source_in, source_out) = get_ping_ports(a, &pattern.source)?;
    for_targets(pattern, instances, |target| {
        let (target_in, target_out) = get_ping_ports(a, target)?;
        // Health component does not ping itself
        if source_out.interface_instance != target_in.interface_instance {
            Ok(vec![
                (
                    "Health".to_string(),
                    connect(loc, source_out.clone(), target_in),
                ),
                (
                    "Health".to_string(),
                    connect(loc, target_out, source_in.clone()),
                ),
            ])
        } else {
            Ok(vec![])
        }
    })
}

/// Resolve a param pattern
fn resolve_param(
    a: &Analysis,
    pattern: &ConnectionPattern,
    instances: &[ComponentInstance],
) -> SemanticResult<Vec<(String, Connection)>> {
    let loc = pattern.loc;
    let prm_get_in = get_general_port(
        a,
        &pattern.source,
        "param get",
        Direction::Input,
        "Fw.PrmGet",
    )?;
    let prm_set_in = get_general_port(
        a,
        &pattern.source,
        "param set",
        Direction::Input,
        "Fw.PrmSet",
    )?;
    for_targets(pattern, instances, |target| {
        let prm_get_out = get_special_port(a, target, SpecialPortInstanceKind::ParamGet)?;
        let prm_set_out = get_special_port(a, target, SpecialPortInstanceKind::ParamSet)?;
        Ok(vec![
            (
                "Parameters".to_string(),
                connect(loc, prm_get_out, prm_get_in.clone()),
            ),
            (
                "Parameters".to_string(),
                connect(loc, prm_set_out, prm_set_in.clone()),
            ),
        ])
    })
}
