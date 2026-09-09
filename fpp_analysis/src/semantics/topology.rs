use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    ComponentInstance, Connection, InterfaceInstance, PortInstance, PortInstanceIdentifier,
    PortInterface, Symbol, SymbolInterface,
};
use fpp_ast::{self as ast, AstNode, ConnectionPatternKind, QualIdent};
use fpp_core::{Node, Span, Spanned};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::collections::{BTreeMap, BTreeSet};

/// A resolved topology port, aliasing an underlying port instance.
#[derive(Debug, Clone)]
pub struct TopologyPort {
    pub name: String,
    pub node_id: Node,
    pub loc: Span,
    /// The underlying port instance identifier.
    pub pii: PortInstanceIdentifier,
    /// The location of the underlying port use.
    pub underlying_loc: Span,
}

/// A top port collected during the topology walk, to be resolved into the port
/// interface later (once imported topologies are resolved).
#[derive(Debug, Clone)]
pub struct PendingTopPort {
    pub name: String,
    pub node_id: Node,
    pub loc: Span,
    pub underlying_ast: ast::PortInstanceIdentifier,
}

/// A resolved connection pattern.
#[derive(Debug, Clone)]
pub struct ConnectionPattern {
    pub loc: Span,
    pub kind: ConnectionPatternKind,
    /// The source instance.
    pub source: (ComponentInstance, Span),
    /// The target instances.
    pub targets: Vec<(ComponentInstance, Span)>,
}

impl ConnectionPattern {
    /// Build a connection pattern from its AST spec. Returns `None` if the
    /// source or any target is unresolved (already reported by CheckUses).
    pub fn from_spec(
        a: &crate::Analysis,
        spec: &ast::SpecPatternConnectionGraph,
    ) -> SemanticResult<Option<ConnectionPattern>> {
        use fpp_core::Spanned;
        let Some(source_ci) = a.get_component_instance(spec.source.id()) else {
            return Ok(None);
        };
        let source = (source_ci, spec.source.span());
        let mut targets = Vec::new();
        for tgt in &spec.targets {
            let Some(ci) = a.get_component_instance(tgt.id()) else {
                return Ok(None);
            };
            targets.push((ci, tgt.span()));
        }
        Ok(Some(ConnectionPattern {
            loc: spec.span(),
            kind: spec.kind.clone(),
            source,
            targets,
        }))
    }
}

/// An FPP topology.
#[derive(Debug, Clone)]
pub struct Topology {
    /// The topology symbol
    pub symbol: Symbol,
    /// The fully qualified name of the topology
    pub name: String,
    /// The location of the topology definition
    pub loc: Span,
    /// The interfaces this topology implements (AST use nodes).
    pub implements: Vec<QualIdent>,
    /// The component instances directly declared in this topology
    pub direct_component_instances: HashMap<Symbol, Span>,
    /// The topologies directly imported into this topology
    pub direct_topologies: HashMap<Symbol, Span>,
    /// The transitively imported topologies (by symbol).
    pub transitive_import_set: HashSet<Symbol>,
    /// The instances of this topology, resolved across imports.
    pub instance_map: BTreeMap<InterfaceInstance, Span>,
    /// The top ports to resolve into the port interface.
    pub ports: Vec<PendingTopPort>,
    /// The raw direct connection graph specs, to resolve in dependency order.
    pub raw_direct_graphs: Vec<ast::SpecDirectConnectionGraph>,
    /// The raw pattern connection graph specs, to resolve in dependency order.
    pub raw_patterns: Vec<ast::SpecPatternConnectionGraph>,
    /// The resolved top ports by name.
    pub port_map: HashMap<String, TopologyPort>,
    /// The resolved port interface of the topology (from its topology ports).
    pub port_interface: PortInterface,
    /// The connection patterns of this topology, indexed by kind.
    pub pattern_map: HashMap<ConnectionPatternKind, ConnectionPattern>,
    /// The connections of this topology, indexed by graph name.
    pub connection_map: BTreeMap<String, Vec<Connection>>,
    /// The connections defined locally (not imported), indexed by graph name.
    pub local_connection_map: BTreeMap<String, Vec<Connection>>,
    /// The output connections going from each port.
    pub output_connection_map: BTreeMap<PortInstanceIdentifier, BTreeSet<Connection>>,
    /// The input connections going to each port.
    pub input_connection_map: BTreeMap<PortInstanceIdentifier, BTreeSet<Connection>>,
    /// The mapping between connections and from port numbers.
    pub from_port_number_map: BTreeMap<Connection, i128>,
    /// The mapping between connections and to port numbers.
    pub to_port_number_map: BTreeMap<Connection, i128>,
    /// The unconnected port instances.
    pub unconnected_port_set: BTreeSet<PortInstanceIdentifier>,
}

impl Topology {
    pub fn new(symbol: Symbol, name: String, loc: Span, implements: Vec<QualIdent>) -> Topology {
        Topology {
            symbol,
            name,
            loc,
            implements,
            direct_component_instances: HashMap::default(),
            direct_topologies: HashMap::default(),
            transitive_import_set: HashSet::default(),
            instance_map: BTreeMap::new(),
            ports: Vec::new(),
            raw_direct_graphs: Vec::new(),
            raw_patterns: Vec::new(),
            port_map: HashMap::default(),
            port_interface: PortInterface::new("topology"),
            pattern_map: HashMap::default(),
            connection_map: BTreeMap::new(),
            local_connection_map: BTreeMap::new(),
            output_connection_map: BTreeMap::new(),
            input_connection_map: BTreeMap::new(),
            from_port_number_map: BTreeMap::new(),
            to_port_number_map: BTreeMap::new(),
            unconnected_port_set: BTreeSet::new(),
        }
    }

    /// The unqualified name of the topology.
    pub fn unqualified_name(&self) -> String {
        self.symbol.name().data.clone()
    }

    /// Add an interface instance symbol (component instance or imported
    /// topology) that must be unique within its category.
    pub fn add_instance_symbol(&mut self, symbol: Symbol, loc: Span) -> SemanticResult {
        let map = match &symbol {
            Symbol::ComponentInstance(_) => &mut self.direct_component_instances,
            Symbol::Topology(def) => {
                // A deployment topology may not be imported into another topology.
                if def.is_deployment {
                    return Err(SemanticError::InvalidSymbol {
                        symbol_name: symbol.name().data.clone(),
                        msg: format!(
                            "invalid use of symbol {}: use of deployment topology is not allowed here",
                            symbol.name().data
                        ),
                        loc,
                        def_loc: symbol.node().span(),
                    });
                }
                &mut self.direct_topologies
            }
            // Other symbol kinds are rejected earlier during use resolution.
            _ => return Ok(()),
        };
        if let Some(prev_loc) = map.get(&symbol) {
            return Err(SemanticError::DuplicateInstance {
                name: symbol.name().data.clone(),
                loc,
                prev_loc: *prev_loc,
            });
        }
        map.insert(symbol, loc);
        Ok(())
    }

    /// Add an instance to the resolved instance map, keeping the earliest loc.
    pub fn add_instance(&mut self, instance: InterfaceInstance, loc: Span) {
        self.instance_map.entry(instance).or_insert(loc);
    }

    /// Add a top port node to be resolved later.
    pub fn add_port_node(&mut self, port: PendingTopPort) {
        self.ports.push(port);
    }

    /// Add a pattern, erroring on duplicate kind.
    pub fn add_pattern(&mut self, pattern: ConnectionPattern) -> SemanticResult {
        if let Some(prev) = self.pattern_map.get(&pattern.kind) {
            return Err(SemanticError::DuplicatePattern {
                kind: pattern_kind_str(&pattern.kind),
                loc: pattern.loc,
                prev_loc: prev.loc,
            });
        }
        self.pattern_map.insert(pattern.kind.clone(), pattern);
        Ok(())
    }

    /// Resolve a top port into the port interface.
    pub fn add_port(
        &mut self,
        name: &str,
        node_id: Node,
        loc: Span,
        underlying: PortInstanceIdentifier,
        underlying_loc: Span,
    ) -> SemanticResult {
        // Check that the topology port is for a general port
        if matches!(underlying.port_instance, PortInstance::Internal { .. }) {
            return Err(SemanticError::InvalidPortInstance {
                loc,
                msg: "topology port cannot point to an internal port".to_string(),
                def_loc: underlying.port_instance.get_loc(),
            });
        }
        let topology_pi = PortInstance::topology(
            node_id,
            loc,
            name.to_string(),
            underlying.port_instance.clone(),
        );
        let new_interface = self.port_interface.add_port_instance(topology_pi)?;
        if let Some(prev) = self.port_map.get(name) {
            return Err(SemanticError::DuplicatePortInstance {
                name: name.to_string(),
                loc,
                import_locs: vec![],
                prev_loc: prev.loc,
                prev_import_locs: vec![],
            });
        }
        self.port_map.insert(
            name.to_string(),
            TopologyPort {
                name: name.to_string(),
                node_id,
                loc,
                pii: underlying,
                underlying_loc,
            },
        );
        self.port_interface = new_interface;
        Ok(())
    }

    /// Add a connection to all the connection maps.
    pub fn add_connection(&mut self, graph_name: &str, c: Connection) {
        self.connection_map
            .entry(graph_name.to_string())
            .or_default()
            .push(c.clone());
        self.output_connection_map
            .entry(c.from.port.clone())
            .or_default()
            .insert(c.clone());
        self.input_connection_map
            .entry(c.to.port.clone())
            .or_default()
            .insert(c.clone());
        if let Some(n) = c.from.port_number {
            self.from_port_number_map.insert(c.clone(), n);
        }
        if let Some(n) = c.to.port_number {
            self.to_port_number_map.insert(c.clone(), n);
        }
    }

    /// Add a locally declared connection.
    pub fn add_local_connection(&mut self, graph_name: &str, c: Connection) {
        self.local_connection_map
            .entry(graph_name.to_string())
            .or_default()
            .push(c.clone());
        self.add_connection(graph_name, c);
    }

    /// Clear all connection maps (used when re-processing to underlying ports).
    pub fn clear_connections(&mut self) {
        self.local_connection_map.clear();
        self.connection_map.clear();
        self.output_connection_map.clear();
        self.input_connection_map.clear();
        self.from_port_number_map.clear();
        self.to_port_number_map.clear();
    }

    /// Assign a port number to a connection at a port instance.
    pub fn assign_port_number(&mut self, pi: &PortInstance, c: &Connection, n: i128) {
        match pi.get_direction() {
            Some(crate::semantics::Direction::Input) => {
                self.to_port_number_map.insert(c.clone(), n);
            }
            _ => {
                self.from_port_number_map.insert(c.clone(), n);
            }
        }
    }

    /// Get the port number of a connection at a port instance.
    pub fn get_port_number(&self, pi: &PortInstance, c: &Connection) -> Option<i128> {
        match pi.get_direction() {
            Some(crate::semantics::Direction::Input) => self.to_port_number_map.get(c).copied(),
            _ => self.from_port_number_map.get(c).copied(),
        }
    }

    /// Get the connections from a port, sorted.
    pub fn get_connections_from(&self, from: &PortInstanceIdentifier) -> Vec<Connection> {
        self.output_connection_map
            .get(from)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the connections to a port, sorted.
    pub fn get_connections_to(&self, to: &PortInstanceIdentifier) -> Vec<Connection> {
        self.input_connection_map
            .get(to)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the connections at a port instance, sorted, by direction.
    pub fn get_connections_at(&self, pii: &PortInstanceIdentifier) -> Vec<Connection> {
        match pii.port_instance.get_direction() {
            Some(crate::semantics::Direction::Input) => self.get_connections_to(pii),
            Some(crate::semantics::Direction::Output) => self.get_connections_from(pii),
            None => vec![],
        }
    }

    /// Get the connections between two ports.
    pub fn get_connections_between(
        &self,
        from: &PortInstanceIdentifier,
        to: &PortInstanceIdentifier,
    ) -> Vec<Connection> {
        self.get_connections_from(from)
            .into_iter()
            .filter(|c| &c.to.port == to)
            .collect()
    }

    /// Whether a connection exists between two ports.
    pub fn connection_exists_between(
        &self,
        from: &PortInstanceIdentifier,
        to: &PortInstanceIdentifier,
    ) -> bool {
        !self.get_connections_between(from, to).is_empty()
    }

    /// Get the set of used port numbers for a port instance over some connections.
    pub fn get_used_port_numbers(&self, pi: &PortInstance, cs: &[Connection]) -> BTreeSet<i128> {
        let mut s = BTreeSet::new();
        for c in cs {
            if let Some(n) = self.get_port_number(pi, c) {
                s.insert(n);
            }
        }
        s
    }

    /// The component instances of this topology, in qualified-name order.
    pub fn component_instance_map(&self) -> Vec<(ComponentInstance, Span)> {
        self.instance_map
            .iter()
            .filter_map(|(ii, loc)| match ii {
                InterfaceInstance::Component(ci) => Some((ci.clone(), *loc)),
                InterfaceInstance::Topology(_) => None,
            })
            .collect()
    }

    /// Look up an interface instance used at a location.
    pub fn look_up_instance_at(&self, instance: &InterfaceInstance, loc: Span) -> SemanticResult {
        if self.instance_map.contains_key(instance) {
            Ok(())
        } else {
            Err(SemanticError::InvalidInterfaceInstance {
                loc,
                instance_name: instance.unqualified_name(),
                top_name: self.unqualified_name(),
            })
        }
    }
}

/// The name of a connection pattern kind.
pub fn pattern_kind_str(kind: &ConnectionPatternKind) -> String {
    match kind {
        ConnectionPatternKind::Command => "command",
        ConnectionPatternKind::Event => "event",
        ConnectionPatternKind::Health => "health",
        ConnectionPatternKind::Param => "param",
        ConnectionPatternKind::Telemetry => "telemetry",
        ConnectionPatternKind::TextEvent => "text event",
        ConnectionPatternKind::Time => "time",
    }
    .to_string()
}
