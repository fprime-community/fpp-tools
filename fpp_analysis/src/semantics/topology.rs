use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    ComponentInstance, Connection, InterfaceInstance, PortInstance, PortInstanceIdentifier,
    PortInterface, Symbol, SymbolInterface,
};
use fpp_ast::{self as ast, AstNode, ConnectionPatternKind, QualIdent};
use fpp_core::{Span, Spanned};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// A resolved topology port, aliasing an underlying port instance.
#[derive(Debug, Clone)]
pub struct TopologyPort {
    pub node: Arc<ast::SpecTopPort>,
    /// The underlying port instance identifier.
    pub pii: PortInstanceIdentifier,
}

impl TopologyPort {
    /// Gets the name of the topology port.
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the topology port.
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Gets the location of the underlying port use.
    pub fn get_underlying_port_loc(&self) -> Span {
        self.node.underlying_port.span()
    }
}

/// A resolved connection pattern.
#[derive(Debug, Clone)]
pub struct ConnectionPattern {
    /// The AST node specifying the pattern.
    pub node: Arc<ast::SpecPatternConnectionGraph>,
    /// The source instance.
    pub source: (ComponentInstance, Span),
    /// The target instances. Scala holds a set here; this list is deduplicated
    /// by instance, but keeps source order.
    pub targets: Vec<(ComponentInstance, Span)>,
}

impl ConnectionPattern {
    /// Gets the location of the pattern.
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// The kind of the pattern.
    pub fn kind(&self) -> &ConnectionPatternKind {
        &self.node.kind
    }

    /// Build a connection pattern from its AST spec. Returns `None` if the
    /// source or any target is unresolved (already reported by CheckUses).
    pub fn from_spec(
        a: &crate::Analysis,
        spec: &Arc<ast::SpecPatternConnectionGraph>,
    ) -> SemanticResult<Option<ConnectionPattern>> {
        let Some(source_ci) = a.get_component_instance(spec.source.id())? else {
            return Ok(None);
        };
        let source = (source_ci, spec.source.span());
        let mut targets: Vec<(ComponentInstance, Span)> = Vec::new();
        for tgt in &spec.targets {
            let Some(ci) = a.get_component_instance(tgt.id())? else {
                return Ok(None);
            };
            if !targets.iter().any(|(prev, _)| prev == &ci) {
                targets.push((ci, tgt.span()));
            }
        }
        Ok(Some(ConnectionPattern {
            node: spec.clone(),
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
    pub qualified_name: String,
    /// The component instances directly declared in this topology
    pub direct_component_instances: HashMap<Symbol, Span>,
    /// The topologies directly imported into this topology
    pub direct_topologies: HashMap<Symbol, Span>,
    /// The transitively imported topologies (by symbol).
    pub transitive_import_set: HashSet<Symbol>,
    /// The instances of this topology, resolved across imports.
    pub instance_map: BTreeMap<InterfaceInstance, Span>,
    /// The top ports to resolve into the port interface.
    pub ports: Vec<Arc<ast::SpecTopPort>>,
    /// The raw direct connection graph specs, to resolve in dependency order.
    pub raw_direct_graphs: Vec<ast::SpecDirectConnectionGraph>,
    /// The raw pattern connection graph specs, to resolve in dependency order.
    pub raw_patterns: Vec<Arc<ast::SpecPatternConnectionGraph>>,
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
    pub fn new(symbol: Symbol, qualified_name: String) -> Topology {
        Topology {
            symbol,
            qualified_name,
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

    /// The AST node defining the topology.
    pub fn node(&self) -> &ast::DefTopology {
        match &self.symbol {
            Symbol::Topology(def) => def,
            // A topology is always built from a topology symbol.
            _ => unreachable!("topology symbol is not a topology"),
        }
    }

    /// Gets the name of the topology.
    pub fn get_name(&self) -> &str {
        &self.node().name.data
    }

    /// The unqualified name of the topology.
    pub fn unqualified_name(&self) -> String {
        self.symbol.name().data.clone()
    }

    /// Gets the location of the topology.
    pub fn get_loc(&self) -> Span {
        self.node().span()
    }

    /// The interfaces this topology implements (AST use nodes).
    pub fn implements(&self) -> &[QualIdent] {
        &self.node().implements
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
    pub fn add_port_node(&mut self, node: Arc<ast::SpecTopPort>) {
        self.ports.push(node);
    }

    /// Add a pattern, erroring on duplicate kind.
    pub fn add_pattern(&mut self, pattern: ConnectionPattern) -> SemanticResult {
        if let Some(prev) = self.pattern_map.get(pattern.kind()) {
            return Err(SemanticError::DuplicatePattern {
                kind: pattern_kind_str(pattern.kind()),
                loc: pattern.get_loc(),
                prev_loc: prev.get_loc(),
            });
        }
        self.pattern_map.insert(pattern.kind().clone(), pattern);
        Ok(())
    }

    /// Resolve a top port into the port interface.
    pub fn add_port(
        &mut self,
        node: Arc<ast::SpecTopPort>,
        underlying_port: PortInstanceIdentifier,
    ) -> SemanticResult {
        let name = node.name.data.clone();
        let loc = node.span();
        // Check that the topology port is for a general port
        if matches!(underlying_port.port_instance, PortInstance::Internal(_)) {
            return Err(SemanticError::InvalidPortInstance {
                loc,
                msg: "topology port cannot point to an internal port".to_string(),
                def_loc: underlying_port.port_instance.get_loc(),
            });
        }
        let topology_pi =
            PortInstance::topology(node.clone(), underlying_port.port_instance.clone());
        let new_interface = self.port_interface.add_port_instance(topology_pi)?;
        if let Some(prev) = self.port_map.get(&name) {
            return Err(SemanticError::DuplicatePortInstance {
                name,
                loc,
                import_locs: vec![],
                prev_loc: prev.get_loc(),
                prev_import_locs: vec![],
            });
        }
        self.port_map.insert(
            name,
            TopologyPort {
                node,
                pii: underlying_port,
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
    pub fn component_instance_map(&self) -> BTreeMap<ComponentInstance, Span> {
        self.instance_map
            .iter()
            .filter_map(|(ii, loc)| ii.get_component_instance_opt().map(|ci| (ci.clone(), *loc)))
            .collect()
    }

    /// Looks up the location where a component instance appears in this topology.
    pub fn look_up_component_instance_loc(&self, ci: &ComponentInstance) -> Option<Span> {
        self.instance_map
            .get(&InterfaceInstance::from_component_instance(ci.clone()))
            .copied()
    }

    /// Resolve the port numbers in a connection.
    ///
    /// Returns a copy of `c` whose endpoints carry the port numbers that port
    /// numbering assigned to it, so that a connection with implicit numbering
    /// reports its resolved numbers instead of `None`.
    pub fn resolve_numbers(&self, c: &Connection) -> Connection {
        let from_port_number = self.get_port_number(&c.from.port.port_instance, c);
        let to_port_number = self.get_port_number(&c.to.port.port_instance, c);
        let mut resolved = c.clone();
        resolved.from.port_number = from_port_number;
        resolved.to.port_number = to_port_number;
        resolved
    }

    /// Sort connections by their resolved port numbers.
    ///
    /// The returned connections are the originals, in the order given by
    /// comparing their [`Self::resolve_numbers`] images.
    pub fn sort_connections(&self, connections: &[Connection]) -> Vec<Connection> {
        let mut pairs: Vec<(Connection, Connection)> = connections
            .iter()
            .map(|c| (c.clone(), self.resolve_numbers(c)))
            .collect();
        pairs.sort_by(|(_, a), (_, b)| a.cmp(b));
        pairs.into_iter().map(|(c, _)| c).collect()
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

#[cfg(test)]
mod tests {
    use super::Topology;
    use crate::semantics::{Connection, InterfaceInstance, PortInstanceIdentifier};
    use crate::{Analysis, add_state_enums, check_semantics};
    use fpp_core::SourceFile;

    /// A topology with one output port array feeding two input ports. The
    /// connections are written in the order (`c3` first, `c2` second) that is
    /// the reverse of the order port numbering assigns to them, so that the
    /// declared order and the resolved-number order differ.
    const SRC: &str = r#"
module M {

  port P

  passive component C1 {

    output port pOut: [2] P

  }

  passive component C2 {

    sync input port pIn: P

  }

  instance c1: C1 base id 0x100
  instance c2: C2 base id 0x200
  instance c3: C2 base id 0x300

  topology T {

    instance c1
    instance c2
    instance c3

    connections C {

      c1.pOut -> c3.pIn
      c1.pOut -> c2.pIn

    }

  }

}
"#;

    /// Analyze `src` and hand the resolved topology named `top_name` to `f`.
    /// Panics if the input produced any diagnostic.
    fn with_topology(src: &str, top_name: &str, f: impl FnOnce(&Topology)) {
        let mut diagnostics = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
        fpp_core::run(&mut ctx, || {
            let source = SourceFile::new("topology_test.fpp", src.to_string());
            let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let mut a = Analysis::new();
            add_state_enums(&mut ast);
            let _ = check_semantics(&mut a, vec![&ast]);
            let top = a
                .topology_map
                .values()
                .find(|t| t.get_name() == top_name)
                .expect("the topology was resolved");
            f(top);
        });
        let output = String::from_utf8(diagnostics).expect("diagnostics are UTF-8");
        assert_eq!(output, "", "expected no diagnostics");
    }

    /// The connections of graph `C`, in declaration order.
    fn declared_connections(top: &Topology) -> Vec<Connection> {
        top.connection_map
            .get("C")
            .expect("graph C was resolved")
            .clone()
    }

    #[test]
    fn resolve_numbers_fills_in_the_assigned_port_numbers() {
        with_topology(SRC, "T", |top| {
            let connections = declared_connections(top);
            assert_eq!(connections.len(), 2);
            for c in &connections {
                // The connections are implicitly numbered, so the endpoints as
                // written carry no port number.
                assert_eq!(c.from.port_number, None);
                assert_eq!(c.to.port_number, None);
                let resolved = top.resolve_numbers(c);
                assert_eq!(
                    resolved.from.port_number,
                    top.get_port_number(&c.from.port.port_instance, c)
                );
                assert_eq!(
                    resolved.to.port_number,
                    top.get_port_number(&c.to.port.port_instance, c)
                );
                // Both endpoints of a resolved connection have a number, and
                // the rest of the connection is unchanged.
                assert!(resolved.from.port_number.is_some());
                assert_eq!(resolved.to.port_number, Some(0));
                assert_eq!(
                    resolved.from.port.qualified_name(),
                    c.from.port.qualified_name()
                );
                assert_eq!(
                    resolved.to.port.qualified_name(),
                    c.to.port.qualified_name()
                );
            }
            // `c1.pOut` has size 2, so the two connections get numbers 0 and 1.
            // The one to `c2.pIn` sorts first, so it gets 0.
            let numbers: Vec<Option<i128>> = connections
                .iter()
                .map(|c| top.resolve_numbers(c).from.port_number)
                .collect();
            assert_eq!(numbers, vec![Some(1), Some(0)]);
        })
    }

    #[test]
    fn sort_connections_orders_by_the_resolved_port_numbers() {
        with_topology(SRC, "T", |top| {
            let connections = declared_connections(top);
            let sorted = top.sort_connections(&connections);
            let names: Vec<String> = sorted.iter().map(|c| c.to.port.qualified_name()).collect();
            assert_eq!(names, vec!["M.c2.pIn".to_string(), "M.c3.pIn".to_string()]);
            // The returned connections are the originals, not their resolved
            // images: their port numbers are still unset.
            assert!(sorted.iter().all(|c| c.from.port_number.is_none()));
            // Sorting is idempotent.
            assert_eq!(top.sort_connections(&sorted), sorted);
        })
    }

    /// A topology whose matched port numbering assigns numbers that run against
    /// the alphabetical order of the peer instances: `pOut1` is numbered
    /// explicitly (`d1` gets 1, `d2` gets 0), and the matched `pOut2`
    /// connections inherit those numbers.
    const MATCHED_SRC: &str = r#"
module M {

  port P

  passive component C1 {

    output port pOut1: [2] P

    output port pOut2: [2] P

    match pOut1 with pOut2

  }

  passive component C2 {

    sync input port pIn: P

  }

  instance c1: C1 base id 0x100
  instance d1: C2 base id 0x200
  instance d2: C2 base id 0x300

  topology U {

    instance c1
    instance d1
    instance d2

    connections C {

      c1.pOut1[1] -> d1.pIn
      c1.pOut1[0] -> d2.pIn
      c1.pOut2 -> d2.pIn
      c1.pOut2 -> d1.pIn

    }

  }

}
"#;

    #[test]
    fn sort_connections_differs_from_sorting_the_connections_as_written() {
        with_topology(MATCHED_SRC, "U", |top| {
            // The two implicitly numbered `pOut2` connections.
            let connections: Vec<Connection> = declared_connections(top)
                .into_iter()
                .filter(|c| c.from.port.get_unqualified_name() == "c1.pOut2")
                .collect();
            assert_eq!(connections.len(), 2);
            let to_names = |cs: &[Connection]| -> Vec<String> {
                cs.iter().map(|c| c.to.port.qualified_name()).collect()
            };

            // As written, both endpoints are unnumbered, so an ordinary sort
            // falls back to comparing the `to` port names.
            let mut as_written = connections.clone();
            as_written.sort();
            assert_eq!(to_names(&as_written), vec!["M.d1.pIn", "M.d2.pIn"]);

            // Matched numbering gave `-> d2.pIn` the number 0 and
            // `-> d1.pIn` the number 1, so the resolved order is the reverse.
            assert_eq!(
                to_names(&top.sort_connections(&connections)),
                vec!["M.d2.pIn", "M.d1.pIn"]
            );
        })
    }

    #[test]
    fn port_instance_identifier_names() {
        with_topology(SRC, "T", |top| {
            let c = &declared_connections(top)[0];
            assert_eq!(c.from.port.qualified_name(), "M.c1.pOut");
            assert_eq!(c.from.port.get_unqualified_name(), "c1.pOut");
            assert_eq!(c.to.port.qualified_name(), "M.c3.pIn");
            assert_eq!(c.to.port.get_unqualified_name(), "c3.pIn");
        })
    }

    /// A topology `A` in module `M` exposing a topology port `a`.
    const TOP_PORT_SRC: &str = r#"
module M {

  port P

  passive component C1 {

    output port pOut: P

  }

  passive component C2 {

    sync input port pIn: P

  }

  instance c1: C1 base id 0x100
  instance c2: C2 base id 0x200

  topology A {

    instance c1
    instance c2

    port a = c1.pOut
    port b = c2.pIn

  }

}
"#;

    #[test]
    fn port_instance_identifier_names_for_a_topology_instance() {
        with_topology(TOP_PORT_SRC, "A", |top| {
            let interface_instance = InterfaceInstance::from_topology(top);
            let port_instance = top
                .port_interface
                .get_port_instance("a", top.get_loc(), &top.unqualified_name())
                .expect("the topology port was resolved");
            let pii = PortInstanceIdentifier {
                interface_instance,
                port_instance,
            };
            // The interface instance is the topology `M.A`, whose unqualified
            // name is `A`.
            assert_eq!(pii.qualified_name(), "M.A.a");
            assert_eq!(pii.get_unqualified_name(), "A.a");
        })
    }
}
