use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    ComponentInstance, Direction, InterfaceInstance, PortInstance, PortInstanceIdentifier,
    PortInstanceType,
};
use fpp_ast as ast;
use fpp_core::{Span, Spanned};
use std::cmp::Ordering;

/// A connection endpoint
#[derive(Debug, Clone)]
pub struct Endpoint {
    /// The location where the endpoint is defined
    pub loc: Span,
    /// The port instance identifier
    pub port: PortInstanceIdentifier,
    /// The port number
    pub port_number: Option<i128>,
    /// Topology port this mapped to
    pub topology_port: Option<Box<Endpoint>>,
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.port_number {
            Some(n) => write!(f, "{}[{}]", self.port.qualified_name(), n),
            None => f.write_str(&self.port.qualified_name()),
        }
    }
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
    /// Creates an endpoint with no port number at a location
    pub fn new(loc: Span, port: PortInstanceIdentifier) -> Endpoint {
        Endpoint {
            loc,
            port,
            port_number: None,
            topology_port: None,
        }
    }

    /// Check that port number is in bounds for the size
    fn check_port_number(&self, loc: Span) -> SemanticResult {
        if let Some(n) = self.port_number {
            let size = self.port.port_instance.get_array_size();
            if n >= size {
                return Err(SemanticError::InvalidPortNumber {
                    loc,
                    port_number: n,
                    port: self.port.qualified_name(),
                    size,
                    spec_loc: self.port.port_instance.get_loc(),
                });
            }
        }
        Ok(())
    }

    /// Get underlying endpoint by stripping off topology port aliases
    /// and template interface parameters
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
            InterfaceInstance::TemplateArg(arg) => {
                // Replace the bound parameter with the instance bound to it,
                // taking the like-named port of that instance
                let bound = (*arg.instance).clone();
                let name = self.port.port_instance.get_unqualified_name();
                let port_instance = bound
                    .get_port_instance(a, name)
                    .unwrap_or_else(|| self.port.port_instance.clone());
                let next = Endpoint {
                    loc: self.loc,
                    port: PortInstanceIdentifier {
                        interface_instance: bound,
                        port_instance,
                    },
                    port_number: self.port_number,
                    topology_port: self.topology_port.clone(),
                };
                next.get_underlying_endpoint(a)
            }
        }
    }

    /// Constructs a connection endpoint from AST info
    ///
    /// Returns `None` if the port instance identifier names an instance that is
    /// unresolved or whose definition failed to check, or if the port number
    /// does not have a numeric type (all already reported).
    pub fn from_ast(
        a: &Analysis,
        port: &ast::PortInstanceIdentifier,
        port_number: &Option<ast::Expr>,
    ) -> SemanticResult<Option<Endpoint>> {
        let Some(pid) = PortInstanceIdentifier::from_node(a, port)? else {
            return Ok(None);
        };
        pid.port_instance.require_connection_at(port.span())?;
        a.check_numeric_type_opt(port_number)?;
        let pn = a.get_nonnegative_int_value_opt(port_number)?;
        let endpoint = Endpoint {
            loc: port.span(),
            port: pid,
            port_number: pn,
            topology_port: None,
        };
        if let Some(pn) = port_number {
            endpoint.check_port_number(pn.span())?;
        }
        Ok(Some(endpoint))
    }
}

/// An FPP connection
#[derive(Debug, Clone)]
pub struct Connection {
    /// The from endpoint
    pub from: Endpoint,
    /// The to endpoint
    pub to: Endpoint,
    /// Whether the connection is marked `unmatched`
    pub is_unmatched: bool,
}

impl std::fmt::Display for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} -> {}", self.from, self.to)
    }
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
    /// Compare two connections
    fn cmp(&self, other: &Self) -> Ordering {
        self.from
            .cmp(&other.from)
            .then_with(|| self.to.cmp(&other.to))
            .then_with(|| self.from.loc.compare(&other.from.loc))
    }
}

impl Connection {
    /// Checks the types of a connection
    fn check_types(&self, a: &Analysis) -> SemanticResult {
        let from_instance = &self.from.port.port_instance;
        let to_instance = &self.to.port.port_instance;
        let from_type = from_instance.get_type();
        let to_type = to_instance.get_type();
        if PortInstanceType::are_compatible(&from_type, &to_type) {
            Ok(())
        } else {
            let msg = format!(
                "cannot connect port types {} and {}",
                PortInstanceType::show(a, &from_type),
                PortInstanceType::show(a, &to_type)
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

    /// Checks the case of a serial port connected to a typed port,
    /// in either direction. The port type may not have a return type.
    fn check_serial_with_typed_input(&self, a: &Analysis) -> SemanticResult {
        let from_instance = &self.from.port.port_instance;
        let to_instance = &self.to.port.port_instance;
        let from_type = from_instance.get_type();
        let to_type = to_instance.get_type();
        match (&from_type, &to_type) {
            (Some(PortInstanceType::Serial), Some(to_def @ PortInstanceType::DefPort(_))) => {
                if let Some(def_loc) = to_def.port_returns_value() {
                    let msg = format!(
                        "cannot connect serial output port to input port of type {}, which returns a value",
                        PortInstanceType::show(a, &to_type)
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
                        PortInstanceType::show(a, &from_type)
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

    /// Checks the directions of a connection
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

    /// Checks whether a connection is match constrained
    ///
    /// Each endpoint is first resolved through any topology-port aliases, so a
    /// connection written against a topology port that aliases a matched
    /// component port is still recognized as match constrained.
    pub(crate) fn is_match_constrained(&self, a: &Analysis) -> bool {
        let port_matching_exists = |ci: &ComponentInstance, pi: &PortInstance| -> bool {
            ci.component(a)
                .port_matching_list
                .iter()
                .any(|pm| pm.matches(pi))
        };
        let is_constrained_at = |endpoint: &Endpoint| -> bool {
            let port = endpoint.get_underlying_endpoint(a).port;
            match &port.interface_instance {
                InterfaceInstance::Component(ci) => port_matching_exists(ci, &port.port_instance),
                _ => false,
            }
        };
        is_constrained_at(&self.from) || is_constrained_at(&self.to)
    }

    /// Creates a connection that is not marked `unmatched`
    pub fn new(from: Endpoint, to: Endpoint) -> Connection {
        Connection {
            from,
            to,
            is_unmatched: false,
        }
    }

    /// Gets the location of the connection
    pub fn get_loc(&self) -> Span {
        self.from.loc
    }

    /// Get this endpoint of a port connection at a port instance
    pub fn get_this_endpoint(&self, pi: &PortInstance) -> &Endpoint {
        match pi.get_direction() {
            Some(Direction::Input) => &self.to,
            _ => &self.from,
        }
    }

    /// Get the other endpoint of a port connection at a port instance
    pub fn get_other_endpoint(&self, pi: &PortInstance) -> &Endpoint {
        match pi.get_direction() {
            Some(Direction::Input) => &self.from,
            _ => &self.to,
        }
    }

    /// Constructs a connection from an AST connection
    ///
    /// Returns `None` if an endpoint names an instance that is unresolved or
    /// whose definition failed to check (both already reported).
    pub fn from_ast(
        a: &Analysis,
        connection: &ast::Connection,
    ) -> SemanticResult<Option<Connection>> {
        let Some(from) = Endpoint::from_ast(a, &connection.from_port, &connection.from_index)?
        else {
            return Ok(None);
        };
        let Some(to) = Endpoint::from_ast(a, &connection.to_port, &connection.to_index)? else {
            return Ok(None);
        };
        let connection = Connection {
            from,
            to,
            is_unmatched: connection.is_unmatched,
        };
        connection.check_directions()?;
        connection.check_types(a)?;
        connection.check_serial_with_typed_input(a)?;
        Ok(Some(connection))
    }
}
