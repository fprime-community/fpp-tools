//! Lowering from [`fpp_analysis::Analysis`] into the framework-agnostic diagram
//! [`crate::ir`].
//!
//! This is the single analysis-consuming step. It reads the already-resolved
//! semantic model (components, instances, resolved connections with port
//! numbering) and produces an [`ir::Diagram`]. It performs no layout and knows
//! nothing about any rendering framework.

use crate::ir::{self, Diagram, DiagramKind, Edge, Node, Port};
use fpp_analysis::Analysis;
use fpp_analysis::semantics::{
    Component, ComponentInstance, Connection, Direction as SemDirection, GeneralKind,
    InterfaceInstance, PortInstance, PortInstanceType, SymbolInterface, Topology,
};

/// Errors that can occur while lowering an element to a diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    /// No element with the requested name and kind was found in the analysis.
    NotFound { kind: DiagramKind, name: String },
    /// The requested connection group does not exist in the topology.
    UnknownConnectionGroup { topology: String, group: String },
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LowerError::NotFound { kind, name } => {
                write!(f, "no {kind:?} named `{name}` found in analysis")
            }
            LowerError::UnknownConnectionGroup { topology, group } => {
                write!(f, "topology `{topology}` has no connection group `{group}`")
            }
        }
    }
}

impl std::error::Error for LowerError {}

/// Lower a component definition (by fully qualified name) into a diagram.
pub fn lower_component(a: &Analysis, name: &str) -> Result<Diagram, LowerError> {
    let component = find_component(a, name).ok_or_else(|| LowerError::NotFound {
        kind: DiagramKind::Component,
        name: name.to_string(),
    })?;

    let qualified_name = a.get_qualified_name(&component.symbol);
    let node = component_def_node(a, component, &qualified_name);

    Ok(Diagram {
        kind: DiagramKind::Component,
        name: qualified_name,
        nodes: vec![node],
        edges: vec![],
    })
}

/// Lower a topology (by fully qualified name) into a diagram of its component
/// instances and all their connections.
pub fn lower_topology(a: &Analysis, name: &str) -> Result<Diagram, LowerError> {
    let topology = find_topology(a, name).ok_or_else(|| LowerError::NotFound {
        kind: DiagramKind::Topology,
        name: name.to_string(),
    })?;

    let nodes = instance_nodes(a, topology);
    let edges = topology_edges(a, topology, None);

    Ok(Diagram {
        kind: DiagramKind::Topology,
        name: topology.qualified_name.clone(),
        nodes,
        edges,
    })
}

/// Lower a single named connection group within a topology into a diagram.
///
/// Only the instances participating in the group's connections are included.
pub fn lower_connection_group(
    a: &Analysis,
    topology_name: &str,
    group: &str,
) -> Result<Diagram, LowerError> {
    let topology = find_topology(a, topology_name).ok_or_else(|| LowerError::NotFound {
        kind: DiagramKind::ConnectionGroup,
        name: topology_name.to_string(),
    })?;

    if !topology.connection_map.contains_key(group) {
        return Err(LowerError::UnknownConnectionGroup {
            topology: topology.qualified_name.clone(),
            group: group.to_string(),
        });
    }

    let edges = topology_edges(a, topology, Some(group));

    // Keep only the instance nodes that participate in this group.
    let used_nodes: rustc_hash::FxHashSet<&str> = edges
        .iter()
        .flat_map(|e| [node_id_of_port(&e.from_port), node_id_of_port(&e.to_port)])
        .collect();

    let nodes = instance_nodes(a, topology)
        .into_iter()
        .filter(|n| used_nodes.contains(n.id.as_str()))
        .collect();

    Ok(Diagram {
        kind: DiagramKind::ConnectionGroup,
        name: format!("{}.{}", topology.qualified_name, group),
        nodes,
        edges,
    })
}

/// Recover the node id from a port id (`<node id>.<port>.<index>`), i.e. strip
/// the final two dot-separated segments.
fn node_id_of_port(port_id: &str) -> &str {
    match (
        port_id.rfind('.'),
        port_id.get(..port_id.rfind('.').unwrap_or(0)),
    ) {
        (Some(_), Some(head)) => match head.rfind('.') {
            Some(cut) => &head[..cut],
            None => head,
        },
        _ => port_id,
    }
}

// --- node construction ------------------------------------------------------

/// Build the single node for a component-definition diagram.
fn component_def_node(a: &Analysis, component: &Component, qualified_name: &str) -> Node {
    Node {
        id: qualified_name.to_string(),
        name: component.symbol.name().data.clone(),
        qualified_name: qualified_name.to_string(),
        class_name: None,
        kind: (&component.node.kind).into(),
        ports: component_ports(a, component, qualified_name),
    }
}

/// Build nodes for every component instance in a topology.
fn instance_nodes(a: &Analysis, topology: &Topology) -> Vec<Node> {
    topology
        .component_instance_map()
        .into_keys()
        .filter_map(|ci| instance_node(a, &ci))
        .collect()
}

/// Build a node for a single component instance, or `None` if its component is
/// unresolved.
fn instance_node(a: &Analysis, ci: &ComponentInstance) -> Option<Node> {
    let component = a.component_map.get(&ci.component_symbol)?;
    Some(Node {
        id: ci.qualified_name.clone(),
        name: ci.get_unqualified_name().to_string(),
        qualified_name: ci.qualified_name.clone(),
        class_name: Some(a.get_qualified_name(&component.symbol)),
        kind: (&component.node.kind).into(),
        ports: component_ports(a, component, &ci.qualified_name),
    })
}

/// Expand all of a component's port instances into per-index [`Port`]s, owned by
/// the node identified by `node_id`.
fn component_ports(a: &Analysis, component: &Component, node_id: &str) -> Vec<Port> {
    let mut ports: Vec<Port> = component
        .port_map()
        .values()
        .flat_map(|pi| expand_port(a, node_id, pi))
        .collect();
    // Deterministic order: by port name, then array index.
    ports.sort_by(|x, y| x.name.cmp(&y.name).then(x.index.cmp(&y.index)));
    ports
}

/// Expand one port instance into one [`Port`] per array index.
fn expand_port(a: &Analysis, node_id: &str, pi: &PortInstance) -> Vec<Port> {
    let name = pi.get_unqualified_name().to_string();
    let direction = match pi.get_direction() {
        Some(SemDirection::Output) => ir::Direction::Output,
        // Internal ports have no direction; treat as input for placement.
        Some(SemDirection::Input) | None => ir::Direction::Input,
    };
    let kind = port_kind(pi);
    let type_name = match pi.get_type() {
        Some(PortInstanceType::DefPort(symbol)) => Some(a.get_qualified_name(&symbol)),
        Some(PortInstanceType::Serial) | None => None,
    };
    let array_size = pi.get_array_size().max(1);

    (0..array_size)
        .map(|index| {
            let label = if array_size > 1 {
                format!("{name}[{index}]")
            } else {
                name.clone()
            };
            Port {
                id: Port::make_id(node_id, &name, index),
                name: name.clone(),
                label,
                direction,
                kind: kind.clone(),
                index,
                array_size,
                type_name: type_name.clone(),
            }
        })
        .collect()
}

/// Classify a port instance into a rendering-relevant [`ir::PortKind`].
fn port_kind(pi: &PortInstance) -> ir::PortKind {
    match pi {
        PortInstance::General(pi) => match pi.kind {
            GeneralKind::AsyncInput { .. } => ir::PortKind::Async,
            GeneralKind::GuardedInput => ir::PortKind::Guarded,
            GeneralKind::SyncInput => ir::PortKind::Sync,
            GeneralKind::Output => ir::PortKind::Output,
        },
        PortInstance::Special(pi) => ir::PortKind::Special(pi.node.kind.to_string()),
        PortInstance::Internal(_) => ir::PortKind::Internal,
        PortInstance::Topology(pi) => port_kind(&pi.underlying_port),
    }
}

// --- edge construction ------------------------------------------------------

/// Build edges for a topology. When `only_group` is `Some`, only connections in
/// that named graph are included; otherwise all connections are included.
fn topology_edges(a: &Analysis, topology: &Topology, only_group: Option<&str>) -> Vec<Edge> {
    let mut edges = Vec::new();
    for (graph_name, connections) in &topology.connection_map {
        if let Some(group) = only_group
            && group != graph_name
        {
            continue;
        }
        for (i, connection) in connections.iter().enumerate() {
            if let Some(edge) = connection_edge(a, topology, graph_name, connection, i) {
                edges.push(edge);
            }
        }
    }
    edges
}

/// Build a single edge from a resolved connection, resolving both endpoints
/// through any topology-port aliases down to the underlying component ports.
fn connection_edge(
    a: &Analysis,
    topology: &Topology,
    graph_name: &str,
    connection: &Connection,
    seq: usize,
) -> Option<Edge> {
    let from_ep = connection.from.get_underlying_endpoint(a);
    let to_ep = connection.to.get_underlying_endpoint(a);

    // Only connections between component instances render as port-to-port edges.
    if !matches!(
        from_ep.port.interface_instance,
        InterfaceInstance::Component(_)
    ) || !matches!(
        to_ep.port.interface_instance,
        InterfaceInstance::Component(_)
    ) {
        return None;
    }

    // The resolved (auto-assigned) port numbers are keyed by the original
    // connection; fall back to any explicit number, then to index 0.
    let from_index = topology
        .from_port_number_map
        .get(connection)
        .copied()
        .or(connection.from.port_number)
        .unwrap_or(0);
    let to_index = topology
        .to_port_number_map
        .get(connection)
        .copied()
        .or(connection.to.port_number)
        .unwrap_or(0);

    let from_port = Port::make_id(
        &from_ep.port.interface_instance.qualified_name(),
        from_ep.port.port_instance.get_unqualified_name(),
        from_index,
    );
    let to_port = Port::make_id(
        &to_ep.port.interface_instance.qualified_name(),
        to_ep.port.port_instance.get_unqualified_name(),
        to_index,
    );

    Some(Edge {
        id: format!("{graph_name}.connection.{seq}"),
        from_port,
        to_port,
        graph_name: graph_name.to_string(),
        unmatched: connection.is_unmatched,
    })
}

// --- lookup helpers ---------------------------------------------------------

/// Find a fully resolved topology by its fully qualified name.
fn find_topology<'a>(a: &'a Analysis, name: &str) -> Option<&'a Topology> {
    a.topology_map.values().find(|t| t.qualified_name == name)
}

/// Find a component by its fully qualified name.
fn find_component<'a>(a: &'a Analysis, name: &str) -> Option<&'a Component> {
    a.component_map
        .values()
        .find(|c| a.get_qualified_name(&c.symbol) == name)
}
