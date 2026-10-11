use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    ComponentInstance, Connection, ConnectionPattern, Direction, InterfaceInstance, PortInstance,
    PortInstanceIdentifier, PortInterface, Symbol, SymbolInterface, TopologyPort,
};
use fpp_ast::{self as ast, ConnectionPatternKind, QualIdent};
use fpp_core::{Span, Spanned};
use indexmap::{IndexMap, IndexSet};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// An FPP topology
#[derive(Debug, Clone)]
pub struct Topology {
    /// The symbol of the topology definition
    pub symbol: Symbol,
    /// Fully qualified name of the topology
    pub qualified_name: String,
    /// The directly imported topologies
    pub direct_topologies: IndexMap<Symbol, Span>,
    /// The directly imported component instances
    pub direct_component_instances: IndexMap<Symbol, Span>,
    /// The directly imported template arguments
    pub direct_template_args: IndexMap<Symbol, Span>,
    /// The transitively imported topologies
    pub transitive_import_set: IndexSet<Symbol>,
    /// The instances of this topology
    pub instance_map: BTreeMap<InterfaceInstance, Span>,
    /// List of the ports in the topology to resolve later
    pub ports: Vec<Arc<ast::SpecTopPort>>,
    /// The direct connection graph specifiers of this topology, in source
    /// order. The analysis does not read them; they are kept for the tools that
    /// present a topology's connection graphs.
    pub raw_direct_graphs: Vec<ast::SpecDirectConnectionGraph>,
    /// The pattern connection graph specifiers of this topology, in source
    /// order. The analysis does not read them; they are kept for the tools that
    /// present a topology's connection graphs.
    pub raw_patterns: Vec<Arc<ast::SpecPatternConnectionGraph>>,
    /// The ports in the topology resolved to their port instance identifiers
    pub port_map: IndexMap<String, TopologyPort>,
    /// Resolved port interface of the topology
    pub port_interface: PortInterface,
    /// The connection patterns of this topology
    pub pattern_map: IndexMap<ConnectionPatternKind, ConnectionPattern>,
    /// The connections of this topology, indexed by graph name. Each list holds
    /// the most recently added connection first.
    pub connection_map: IndexMap<String, Vec<Connection>>,
    /// The connections of this topology marked as unmatched, in the order they
    /// were added
    pub unmatched_connection_set: Vec<Connection>,
    /// The connections defined locally, not imported. Each list holds the most
    /// recently added connection first.
    pub local_connection_map: IndexMap<String, Vec<Connection>>,
    /// The output connections going from each port
    pub output_connection_map: BTreeMap<PortInstanceIdentifier, BTreeSet<Connection>>,
    /// The input connections going to each port
    pub input_connection_map: BTreeMap<PortInstanceIdentifier, BTreeSet<Connection>>,
    /// The mapping between connections and from port numbers
    pub from_port_number_map: BTreeMap<Connection, i128>,
    /// The mapping between connections and to port numbers
    pub to_port_number_map: BTreeMap<Connection, i128>,
    /// The unconnected port instances
    pub unconnected_port_set: BTreeSet<PortInstanceIdentifier>,
}

impl Topology {
    /// Creates a topology with no members from the symbol of its definition
    pub fn new(symbol: Symbol, qualified_name: String) -> Topology {
        Topology {
            symbol,
            qualified_name,
            direct_topologies: IndexMap::default(),
            direct_component_instances: IndexMap::default(),
            direct_template_args: IndexMap::default(),
            transitive_import_set: IndexSet::default(),
            instance_map: BTreeMap::new(),
            ports: Vec::new(),
            raw_direct_graphs: Vec::new(),
            raw_patterns: Vec::new(),
            port_map: IndexMap::default(),
            port_interface: PortInterface::new("topology"),
            pattern_map: IndexMap::default(),
            connection_map: IndexMap::default(),
            unmatched_connection_set: Vec::new(),
            local_connection_map: IndexMap::default(),
            output_connection_map: BTreeMap::new(),
            input_connection_map: BTreeMap::new(),
            from_port_number_map: BTreeMap::new(),
            to_port_number_map: BTreeMap::new(),
            unconnected_port_set: BTreeSet::new(),
        }
    }

    /// The AST node defining the topology
    pub fn node(&self) -> &ast::DefTopology {
        match &self.symbol {
            Symbol::Topology(def) => def,
            // A topology is always built from a topology symbol
            _ => unreachable!("topology symbol is not a topology"),
        }
    }

    /// Gets the name of the topology
    pub fn get_name(&self) -> &str {
        &self.node().name.data
    }

    /// Gets the symbols of the topologies imported into this topology.
    ///
    /// A topology may be imported through a bound port interface instance
    /// template parameter, so each bound parameter is followed to the
    /// definition it represents.
    pub fn get_imported_topology_symbols(&self, a: &Analysis) -> Vec<Symbol> {
        let mut out: Vec<Symbol> = self.direct_topologies.keys().cloned().collect();
        out.extend(
            self.direct_template_args
                .keys()
                .filter_map(|symbol| a.get_represented_topology_symbol(symbol).cloned()),
        );
        out
    }

    /// Add a port to the topology
    pub fn add_port_node(&mut self, node: Arc<ast::SpecTopPort>) {
        self.ports.push(node);
    }

    /// Add a port to the topology
    pub fn add_port(
        &mut self,
        node: Arc<ast::SpecTopPort>,
        underlying_port: PortInstanceIdentifier,
    ) -> SemanticResult {
        let name = node.name.data.clone();
        let loc = node.span();
        // Check that the topology port for a general port
        if matches!(underlying_port.port_instance, PortInstance::Internal(_)) {
            return Err(SemanticError::InvalidPortInstance {
                loc,
                msg: "topology port cannot point to an internal port".to_string(),
                def_loc: underlying_port.port_instance.get_loc(),
            });
        }
        let topology_pi =
            PortInstance::topology(node.clone(), underlying_port.port_instance.clone());
        let pi = self.port_interface.add_port_instance(topology_pi)?;
        if let Some(prev_port) = self.port_map.get(&name) {
            return Err(SemanticError::DuplicatePortInstance {
                name,
                loc,
                import_locs: vec![],
                prev_loc: prev_port.get_loc(),
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
        self.port_interface = pi;
        Ok(())
    }

    /// Add a connection
    pub fn add_connection(&mut self, graph_name: &str, connection: Connection) {
        self.connection_map
            .entry(graph_name.to_string())
            .or_default()
            .insert(0, connection.clone());
        if connection.is_unmatched && !self.unmatched_connection_set.contains(&connection) {
            self.unmatched_connection_set.push(connection.clone());
        }
        self.output_connection_map
            .entry(connection.from.port.clone())
            .or_default()
            .insert(connection.clone());
        self.input_connection_map
            .entry(connection.to.port.clone())
            .or_default()
            .insert(connection.clone());
        if let Some(n) = connection.from.port_number {
            self.from_port_number_map.insert(connection.clone(), n);
        }
        if let Some(n) = connection.to.port_number {
            self.to_port_number_map.insert(connection.clone(), n);
        }
    }

    /// Add a local connection
    pub fn add_local_connection(&mut self, graph_name: &str, connection: Connection) {
        self.local_connection_map
            .entry(graph_name.to_string())
            .or_default()
            .insert(0, connection.clone());
        self.add_connection(graph_name, connection);
    }

    /// Add a pattern
    pub fn add_pattern(
        &mut self,
        kind: &ConnectionPatternKind,
        pattern: ConnectionPattern,
    ) -> SemanticResult {
        match self.pattern_map.get(kind) {
            Some(prev_pattern) => Err(SemanticError::DuplicatePattern {
                kind: kind.to_string(),
                loc: pattern.get_loc(),
                prev_loc: prev_pattern.get_loc(),
            }),
            None => {
                self.pattern_map.insert(kind.clone(), pattern);
                Ok(())
            }
        }
    }

    /// Add an instance that may already be there
    pub fn add_instance(&mut self, instance: InterfaceInstance, loc: Span) {
        // Use the previous location, if it exists
        self.instance_map.entry(instance).or_insert(loc);
    }

    /// Add an instance that must be unique
    pub fn add_instance_symbol(
        &mut self,
        a: &Analysis,
        symbol: Symbol,
        loc: Span,
    ) -> SemanticResult {
        // Check that the symbol is not already an instance of this topology
        let check_not_duplicate = |prev_loc_opt: Option<&Span>| match prev_loc_opt {
            Some(prev_loc) => Err(SemanticError::DuplicateInstance {
                name: symbol.name().data.clone(),
                loc,
                prev_loc: *prev_loc,
            }),
            None => Ok(()),
        };
        // Check that the symbol does not represent a deployment topology.
        // The symbol may be a bound template parameter, so resolve it first.
        if let Some(top @ Symbol::Topology(def)) = a.get_represented_topology_symbol(&symbol)
            && def.is_deployment
        {
            return Err(SemanticError::InvalidSymbol {
                symbol_name: symbol.name().data.clone(),
                msg: "use of deployment topology is not allowed here".to_string(),
                loc,
                def_loc: top.node().span(),
            });
        }
        let map = match &symbol {
            Symbol::ComponentInstance(_) => &mut self.direct_component_instances,
            Symbol::Topology(_) => &mut self.direct_topologies,
            Symbol::TemplateInterfaceArg { .. } => &mut self.direct_template_args,
            _ => {
                unreachable!("the symbol of an instance specifier is an interface instance symbol")
            }
        };
        check_not_duplicate(map.get(&symbol))?;
        map.insert(symbol, loc);
        Ok(())
    }

    /// Assigns a port number to a connection at a port instance
    pub fn assign_port_number(&mut self, pi: &PortInstance, c: &Connection, n: i128) {
        match pi.get_direction() {
            Some(Direction::Input) => {
                self.to_port_number_map.insert(c.clone(), n);
            }
            _ => {
                self.from_port_number_map.insert(c.clone(), n);
            }
        }
    }

    /// Get the port number of a connection at a port instance
    pub fn get_port_number(&self, pi: &PortInstance, c: &Connection) -> Option<i128> {
        match pi.get_direction() {
            Some(Direction::Input) => self.to_port_number_map.get(c).copied(),
            _ => self.from_port_number_map.get(c).copied(),
        }
    }

    /// Check whether a connection exists between two ports
    pub fn connection_exists_between(
        &self,
        from: &PortInstanceIdentifier,
        to: &PortInstanceIdentifier,
    ) -> bool {
        !self.get_connections_between(from, to).is_empty()
    }

    /// Get the connections between two ports, sorted
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

    /// Get the connections from a port, sorted
    pub fn get_connections_from(&self, from: &PortInstanceIdentifier) -> Vec<Connection> {
        self.output_connection_map
            .get(from)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the connections to a port, sorted
    pub fn get_connections_to(&self, to: &PortInstanceIdentifier) -> Vec<Connection> {
        self.input_connection_map
            .get(to)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the connections at a port instance, sorted
    pub fn get_connections_at(&self, pii: &PortInstanceIdentifier) -> Vec<Connection> {
        match pii.port_instance.get_direction() {
            Some(Direction::Input) => self.get_connections_to(pii),
            Some(Direction::Output) => self.get_connections_from(pii),
            None => vec![],
        }
    }

    /// Gets the qualified name of the topology
    pub fn get_qualified_name(&self) -> &str {
        &self.qualified_name
    }

    /// Gets the unqualified name of the topology
    pub fn unqualified_name(&self) -> String {
        self.symbol.name().data.clone()
    }

    /// Gets the location of the topology
    pub fn get_loc(&self) -> Span {
        self.node().span()
    }

    /// The uses of the interfaces this topology implements
    pub fn implements(&self) -> &[QualIdent] {
        &self.node().implements
    }

    /// The set of component instances in the topology
    pub fn component_instance_map(&self) -> BTreeMap<ComponentInstance, Span> {
        self.instance_map
            .iter()
            .filter_map(|(ii, loc)| ii.get_component_instance_opt().map(|ci| (ci.clone(), *loc)))
            .collect()
    }

    /// Looks up the location where a component instance appears in this
    /// topology. The instance may appear directly or through a template
    /// interface parameter.
    pub fn look_up_component_instance_loc(&self, ci: &ComponentInstance) -> Option<Span> {
        self.component_instance_map().get(ci).copied()
    }

    /// Look up a component instance used at a location. The instance may
    /// appear directly, through an imported subtopology, or through a bound
    /// interface template parameter.
    pub fn look_up_component_instance_at(
        &self,
        ci: &ComponentInstance,
        loc: Span,
    ) -> SemanticResult<Span> {
        match self.look_up_component_instance_loc(ci) {
            Some(result) => Ok(result),
            None => Err(SemanticError::InvalidInterfaceInstance {
                loc,
                instance_name: ci.get_unqualified_name().to_string(),
                top_name: self.unqualified_name(),
            }),
        }
    }

    /// Gets the set of used port numbers
    pub fn get_used_port_numbers(&self, pi: &PortInstance, cs: &[Connection]) -> BTreeSet<i128> {
        cs.iter()
            .filter_map(|c| self.get_port_number(pi, c))
            .collect()
    }

    /// Look up a interface instance used at a location
    pub fn look_up_instance_at(&self, instance: &InterfaceInstance, loc: Span) -> SemanticResult {
        match self.instance_map.get(instance) {
            Some(_) => Ok(()),
            None => Err(SemanticError::InvalidInterfaceInstance {
                loc,
                instance_name: instance.unqualified_name(),
                top_name: self.unqualified_name(),
            }),
        }
    }

    /// Resolve the numbers in a connection
    ///
    /// Returns a copy of `c` whose endpoints carry the port numbers that port
    /// numbering assigned to it, so that a connection with implicit numbering
    /// reports its resolved numbers instead of `None`.
    pub fn resolve_numbers(&self, c: &Connection) -> Connection {
        let from_number = self.get_port_number(&c.from.port.port_instance, c);
        let to_number = self.get_port_number(&c.to.port.port_instance, c);
        let mut resolved = c.clone();
        resolved.from.port_number = from_number;
        resolved.to.port_number = to_number;
        resolved
    }

    /// Sort connections
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
}

#[cfg(test)]
mod tests {
    use super::Topology;
    use crate::semantics::{Connection, InterfaceInstance, PortInstanceIdentifier};
    use crate::{Analysis, add_state_enums, check_semantics, resolve_templates};
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
            let _ = resolve_templates(&mut a, std::slice::from_mut(&mut ast));
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
                .get_port_instance("a")
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
