use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    ComponentInstance, Direction, Interface, PortInstance, PortInstanceType, PortInterface, Symbol,
    SymbolInterface, Topology,
};
use fpp_ast::{self as ast, AstNode};
use fpp_core::{Span, Spanned};
use std::cmp::Ordering;

/// Compare two spans deterministically by (file path, start byte position),
/// used for connection sorting.
pub fn cmp_span(a: &Span, b: &Span) -> Ordering {
    let fa = format!("{}", a.file());
    let fb = format!("{}", b.file());
    fa.cmp(&fb)
        .then_with(|| a.start().pos().cmp(&b.start().pos()))
}

/// An imported topology used as an interface instance.
///
/// This stores identity only (symbol + resolved name + location). The full
/// [`Topology`] is looked up lazily from [`Analysis::topology_map`] when its
/// port interface or top-port map is actually needed. Embedding a whole
/// `Topology` here would make cloning/dropping a single [`Connection`]
/// deep-copy every transitively imported topology, which dominated analysis
/// time for deeply-imported models.
#[derive(Debug, Clone)]
pub struct TopologyInstance {
    /// The topology symbol, used to look up the resolved `Topology`.
    pub symbol: Symbol,
    /// The fully qualified name of the topology. Cached, because computing it
    /// needs the enclosing module scope, which is not on the node.
    pub qualified_name: String,
}

/// An FPP interface instance: a component instance or an imported topology.

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum InterfaceInstance {
    Component(ComponentInstance),
    Topology(TopologyInstance),
}

impl InterfaceInstance {
    /// The fully qualified name of the interface instance.
    pub fn qualified_name(&self) -> String {
        match self {
            InterfaceInstance::Component(ci) => ci.qualified_name.clone(),
            InterfaceInstance::Topology(t) => t.qualified_name.clone(),
        }
    }

    /// The unqualified name of the interface instance.
    pub fn unqualified_name(&self) -> String {
        match self {
            InterfaceInstance::Component(ci) => ci.get_unqualified_name().to_string(),
            InterfaceInstance::Topology(t) => t.symbol.name().data.clone(),
        }
    }

    /// The location of the interface instance.
    pub fn get_loc(&self) -> Span {
        match self {
            InterfaceInstance::Component(ci) => ci.get_loc(),
            InterfaceInstance::Topology(t) => t.symbol.node().span(),
        }
    }

    /// The full port interface of this instance. Returns `None` if the
    /// underlying component or topology is not (yet) resolved.
    pub fn get_interface<'a>(&self, a: &'a Analysis) -> Option<&'a PortInterface> {
        match self {
            InterfaceInstance::Component(ci) => ci.get_interface(a),
            InterfaceInstance::Topology(t) => {
                a.topology_map.get(&t.symbol).map(|top| &top.port_interface)
            }
        }
    }

    /// The underlying component instance of this interface instance, or `None`
    /// if it is a topology.
    pub fn get_component_instance_opt(&self) -> Option<&ComponentInstance> {
        match self {
            InterfaceInstance::Component(ci) => Some(ci),
            InterfaceInstance::Topology(_) => None,
        }
    }

    /// Look up the resolved topology this instance refers to, if any.
    ///
    /// Returns `None` for component instances or if the topology is not (yet)
    /// resolved in the analysis.
    pub fn as_topology<'a>(&self, a: &'a Analysis) -> Option<&'a Topology> {
        match self {
            InterfaceInstance::Component(_) => None,
            InterfaceInstance::Topology(t) => a.topology_map.get(&t.symbol),
        }
    }

    /// Look up a port instance by name in this interface instance.
    pub fn get_port_instance(
        &self,
        a: &Analysis,
        name: &ast::Ident,
    ) -> SemanticResult<PortInstance> {
        let interface = self
            .get_interface(a)
            .expect("interface instance references a resolved component or topology");
        interface.get_port_instance(&name.data, name.span(), &self.unqualified_name())
    }
}

impl InterfaceInstance {
    pub fn from_component_instance(ci: ComponentInstance) -> InterfaceInstance {
        InterfaceInstance::Component(ci)
    }

    pub fn from_topology(top: &Topology) -> InterfaceInstance {
        InterfaceInstance::Topology(TopologyInstance {
            symbol: top.symbol.clone(),
            qualified_name: top.qualified_name.clone(),
        })
    }
}

impl PartialEq for InterfaceInstance {
    fn eq(&self, other: &Self) -> bool {
        self.qualified_name() == other.qualified_name()
    }
}
impl Eq for InterfaceInstance {}
impl PartialOrd for InterfaceInstance {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for InterfaceInstance {
    fn cmp(&self, other: &Self) -> Ordering {
        self.qualified_name().cmp(&other.qualified_name())
    }
}

/// A resolved FPP port instance identifier.
#[derive(Debug, Clone)]
pub struct PortInstanceIdentifier {
    /// The interface instance.
    pub interface_instance: InterfaceInstance,
    /// The port instance.
    pub port_instance: PortInstance,
}

impl PartialEq for PortInstanceIdentifier {
    fn eq(&self, other: &Self) -> bool {
        self.qualified_name() == other.qualified_name()
    }
}
impl Eq for PortInstanceIdentifier {}
impl PartialOrd for PortInstanceIdentifier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for PortInstanceIdentifier {
    /// Compare two port instance identifiers.
    fn cmp(&self, other: &Self) -> Ordering {
        self.qualified_name().cmp(&other.qualified_name())
    }
}

impl PortInstanceIdentifier {
    /// The qualified name (instance qualified name + "." + port name).
    pub fn qualified_name(&self) -> String {
        format!(
            "{}.{}",
            self.interface_instance.qualified_name(),
            self.port_instance.get_unqualified_name()
        )
    }

    /// The unqualified name (instance unqualified name + "." + port name).
    pub fn get_unqualified_name(&self) -> String {
        format!(
            "{}.{}",
            self.interface_instance.unqualified_name(),
            self.port_instance.get_unqualified_name()
        )
    }

    /// Build a port instance identifier from an AST node. Returns `None` if the
    /// interface instance is unresolved (already reported by CheckUses).
    pub fn from_node(
        a: &Analysis,
        node: &ast::PortInstanceIdentifier,
    ) -> SemanticResult<Option<PortInstanceIdentifier>> {
        let Some(interface_instance) = a.get_interface_instance(node.interface_instance.id())
        else {
            return Ok(None);
        };
        let port_instance = interface_instance.get_port_instance(a, &node.port_name)?;
        Ok(Some(PortInstanceIdentifier {
            interface_instance,
            port_instance,
        }))
    }
}

/// A connection endpoint.
#[derive(Debug, Clone)]
pub struct Endpoint {
    /// Location where the endpoint is written
    pub loc: Span,
    /// The resolved port instance identifier
    pub port: PortInstanceIdentifier,
    /// The explicit port number, if any
    pub port_number: Option<i128>,
    /// Topology port this endpoint mapped to, if resolved through an alias
    pub topology_port: Option<Box<Endpoint>>,
}

impl PartialEq for Endpoint {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Endpoint {}
impl PartialOrd for Endpoint {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Endpoint {
    fn cmp(&self, other: &Self) -> Ordering {
        let name_cmp = self.port.qualified_name().cmp(&other.port.qualified_name());
        if name_cmp != Ordering::Equal {
            return name_cmp;
        }
        match (self.port_number, other.port_number) {
            (Some(a), Some(b)) => a.cmp(&b),
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
        }
    }
}

impl Endpoint {
    /// Build an endpoint from AST info. Returns `None` if unresolved.
    pub fn from_node(
        a: &Analysis,
        port: &ast::PortInstanceIdentifier,
        index: &Option<ast::Expr>,
    ) -> SemanticResult<Option<Endpoint>> {
        let Some(pid) = PortInstanceIdentifier::from_node(a, port)? else {
            return Ok(None);
        };
        pid.port_instance.require_connection_at(port.span())?;
        let port_number = a.get_nonnegative_big_int_value_opt(index)?;
        let endpoint = Endpoint {
            loc: port.span(),
            port: pid,
            port_number,
            topology_port: None,
        };
        if let Some(idx) = index {
            endpoint.check_port_number(idx.span())?;
        }
        Ok(Some(endpoint))
    }

    /// Build an endpoint directly from a resolved identifier at a location.
    pub fn new(loc: Span, port: PortInstanceIdentifier) -> Endpoint {
        Endpoint {
            loc,
            port,
            port_number: None,
            topology_port: None,
        }
    }

    /// Resolve this endpoint through topology-port aliases to the underlying
    /// component-instance port.
    pub fn get_underlying_endpoint(&self, a: &Analysis) -> Endpoint {
        match &self.port.interface_instance {
            // This endpoint is already a component instance
            InterfaceInstance::Component(_) => self.clone(),
            InterfaceInstance::Topology(_) => {
                let Some(top) = self.port.interface_instance.as_topology(a) else {
                    return self.clone();
                };
                // Look up the mapping for this port instance
                let name = self.port.port_instance.get_unqualified_name();
                match top.port_map.get(name) {
                    Some(tp) => {
                        let next = Endpoint {
                            loc: self.loc,
                            port: tp.pii.clone(),
                            port_number: self.port_number,
                            topology_port: Some(Box::new(self.clone())),
                        };
                        // Recursively resolve the endpoint to a component instance
                        next.get_underlying_endpoint(a)
                    }
                    None => self.clone(),
                }
            }
        }
    }

    /// Check that an explicit port number is within bounds.
    fn check_port_number(&self, loc: Span) -> SemanticResult {
        if let Some(n) = self.port_number {
            let size = self.port.port_instance.get_array_size();
            if n >= size {
                return Err(SemanticError::InvalidPortNumber {
                    loc,
                    port_number: n,
                    port: self.port.qualified_name(),
                    array_size: size,
                    spec_loc: self.port.port_instance.get_loc(),
                });
            }
        }
        Ok(())
    }
}

/// A resolved FPP connection.
#[derive(Debug, Clone)]
pub struct Connection {
    /// The from endpoint.
    pub from: Endpoint,
    /// The to endpoint.
    pub to: Endpoint,
    pub is_unmatched: bool,
}

impl PartialEq for Connection {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Connection {}
impl PartialOrd for Connection {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Connection {
    /// Compare two connections.
    fn cmp(&self, other: &Self) -> Ordering {
        self.from
            .cmp(&other.from)
            .then_with(|| self.to.cmp(&other.to))
            .then_with(|| cmp_span(&self.from.loc, &other.from.loc))
    }
}

impl Connection {
    /// Construct a connection from resolved endpoints (no checks).
    pub fn new(from: Endpoint, to: Endpoint) -> Connection {
        Connection {
            from,
            to,
            is_unmatched: false,
        }
    }

    /// The location of the connection (its from endpoint).
    pub fn get_loc(&self) -> Span {
        self.from.loc
    }

    /// Get this endpoint of a connection at a port instance (by direction).
    pub fn get_this_endpoint(&self, pi: &PortInstance) -> &Endpoint {
        match pi.get_direction() {
            Some(Direction::Input) => &self.to,
            _ => &self.from,
        }
    }

    /// Get the other endpoint of a connection at a port instance (by direction).
    pub fn get_other_endpoint(&self, pi: &PortInstance) -> &Endpoint {
        match pi.get_direction() {
            Some(Direction::Input) => &self.from,
            _ => &self.to,
        }
    }

    /// Build a connection from an AST connection. Returns `None` if unresolved.
    pub fn from_node(a: &Analysis, conn: &ast::Connection) -> SemanticResult<Option<Connection>> {
        let Some(from) = Endpoint::from_node(a, &conn.from_port, &conn.from_index)? else {
            return Ok(None);
        };
        let Some(to) = Endpoint::from_node(a, &conn.to_port, &conn.to_index)? else {
            return Ok(None);
        };
        let connection = Connection {
            from,
            to,
            is_unmatched: conn.is_unmatched,
        };
        connection.check_directions()?;
        connection.check_types()?;
        connection.check_serial_with_typed_input()?;
        if !connection.is_match_constrained(a) && connection.is_unmatched {
            return Err(SemanticError::MissingPortMatching {
                loc: connection.get_loc(),
            });
        }
        Ok(Some(connection))
    }

    /// Check that the connection goes output -> input.
    fn check_directions(&self) -> SemanticResult {
        let from_instance = &self.from.port.port_instance;
        let to_instance = &self.to.port.port_instance;
        let from_dir = from_instance.get_direction();
        let to_dir = to_instance.get_direction();
        if Direction::are_compatible(&from_dir, &to_dir) {
            Ok(())
        } else {
            let msg = format!(
                "invalid directions {} -> {} (should be output -> input)",
                Direction::show(&from_dir),
                Direction::show(&to_dir)
            );
            Err(SemanticError::InvalidConnection {
                loc: self.get_loc(),
                msg,
                from_loc: from_instance.get_loc(),
                to_loc: to_instance.get_loc(),
                from_port_def_loc: None,
                to_port_def_loc: None,
            })
        }
    }

    /// Check that the connection's port types are compatible.
    fn check_types(&self) -> SemanticResult {
        let from_instance = &self.from.port.port_instance;
        let to_instance = &self.to.port.port_instance;
        let from_type = from_instance.get_type();
        let to_type = to_instance.get_type();
        if PortInstanceType::are_compatible(&from_type, &to_type) {
            Ok(())
        } else {
            let msg = format!(
                "cannot connect port types {} and {}",
                PortInstanceType::show(&from_type),
                PortInstanceType::show(&to_type)
            );
            Err(SemanticError::InvalidConnection {
                loc: self.get_loc(),
                msg,
                from_loc: from_instance.get_loc(),
                to_loc: to_instance.get_loc(),
                from_port_def_loc: None,
                to_port_def_loc: None,
            })
        }
    }

    /// Check the case of a serial port connected to a typed port that returns a
    /// value, in either direction.
    fn check_serial_with_typed_input(&self) -> SemanticResult {
        let from_instance = &self.from.port.port_instance;
        let to_instance = &self.to.port.port_instance;
        let from_type = from_instance.get_type();
        let to_type = to_instance.get_type();
        match (&from_type, &to_type) {
            (Some(PortInstanceType::Serial), Some(to_def @ PortInstanceType::DefPort(_))) => {
                if let Some(def_loc) = to_def.port_returns_value() {
                    let msg = format!(
                        "cannot connect serial output port to input port of type {}, which returns a value",
                        PortInstanceType::show(&to_type)
                    );
                    return Err(SemanticError::InvalidConnection {
                        loc: self.get_loc(),
                        msg,
                        from_loc: from_instance.get_loc(),
                        to_loc: to_instance.get_loc(),
                        from_port_def_loc: None,
                        to_port_def_loc: Some(def_loc),
                    });
                }
            }
            (Some(from_def @ PortInstanceType::DefPort(_)), Some(PortInstanceType::Serial)) => {
                if let Some(def_loc) = from_def.port_returns_value() {
                    let msg = format!(
                        "cannot connect output port of type {}, which returns a value, to serial input port",
                        PortInstanceType::show(&from_type)
                    );
                    return Err(SemanticError::InvalidConnection {
                        loc: self.get_loc(),
                        msg,
                        from_loc: from_instance.get_loc(),
                        to_loc: to_instance.get_loc(),
                        from_port_def_loc: Some(def_loc),
                        to_port_def_loc: None,
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Whether either endpoint's port participates in a port matching on its
    /// component. Used to validate `unmatched` connections.
    ///
    /// Each endpoint is first resolved through any topology-port aliases, so a
    /// connection written against a topology port that aliases a matched
    /// component port is still recognized as match constrained.
    fn is_match_constrained(&self, a: &Analysis) -> bool {
        let port_matching_exists = |ci: &ComponentInstance, pi: &PortInstance| -> bool {
            match a.component_map.get(&ci.component_symbol) {
                Some(comp) => comp.port_matching_list.iter().any(|pm| pm.matches(pi)),
                None => false,
            }
        };
        let is_constrained_at = |endpoint: &Endpoint| -> bool {
            let port = endpoint.get_underlying_endpoint(a).port;
            match &port.interface_instance {
                InterfaceInstance::Component(ci) => port_matching_exists(ci, &port.port_instance),
                InterfaceInstance::Topology(_) => false,
            }
        };
        is_constrained_at(&self.from) || is_constrained_at(&self.to)
    }
}

impl Analysis {
    /// Resolve a use node to an interface instance
    pub fn get_interface_instance(&self, id: fpp_core::Node) -> Option<InterfaceInstance> {
        match self.use_def_map.get(&id) {
            Some(symbol @ Symbol::ComponentInstance(_)) => self
                .component_instance_map
                .get(symbol)
                .cloned()
                .map(InterfaceInstance::Component),
            Some(symbol @ Symbol::Topology(_)) => self
                .topology_map
                .get(symbol)
                .map(InterfaceInstance::from_topology),
            _ => None,
        }
    }

    // Resolve a use (by node ID) to a component instance
    pub fn get_component_instance(
        &self,
        id: fpp_core::Node,
    ) -> SemanticResult<Option<ComponentInstance>> {
        match self.use_def_map.get(&id) {
            Some(symbol @ Symbol::ComponentInstance(_)) => {
                Ok(self.component_instance_map.get(symbol).cloned())
            }
            Some(symbol) => Err(SemanticError::InvalidSymbol {
                symbol_name: symbol.name().data.clone(),
                msg: format!(
                    "invalid use of symbol {}: not a component instance symbol",
                    symbol.name().data
                ),
                loc: id.span(),
                def_loc: symbol.node().span(),
            }),
            // Unresolved use: CheckUses already reported the error.
            None => Ok(None),
        }
    }

    /// Resolve a use node to an interface.
    pub fn get_interface(&self, id: fpp_core::Node) -> Option<Interface> {
        match self.use_def_map.get(&id) {
            Some(symbol @ Symbol::Interface(_)) => self.interface_map.get(symbol).cloned(),
            _ => None,
        }
    }

    /// Whether a port instance is a general port with the given direction and
    /// fully qualified port type name.
    pub fn is_general_port(
        &self,
        pi: &PortInstance,
        direction: Direction,
        port_type_name: &str,
    ) -> bool {
        match (pi.get_type(), pi.get_direction()) {
            (Some(PortInstanceType::DefPort(symbol)), Some(d)) => {
                self.get_qualified_name(&symbol) == port_type_name && d == direction
            }
            _ => false,
        }
    }
}
