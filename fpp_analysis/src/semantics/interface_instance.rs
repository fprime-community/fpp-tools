use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    ComponentInstance, Interface, PortInstance, PortInstanceIdentifier, PortInterface, Symbol,
    SymbolInterface, Topology,
};
use fpp_ast::{self as ast, AstNode, Ident};
use fpp_core::{Span, Spanned};
use std::cmp::Ordering;
use std::sync::Arc;

/// An FPP interface instance: a component instance, an imported topology, or a
/// bound port interface instance template parameter.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum InterfaceInstance {
    Component(ComponentInstance),
    Topology(TopologyInstance),
    TemplateArg(TemplateArgInstance),
}

impl std::fmt::Display for InterfaceInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.qualified_name())
    }
}

impl InterfaceInstance {
    /// Gets the qualified name of the interface instance
    pub fn qualified_name(&self) -> String {
        match self {
            InterfaceInstance::Component(ci) => ci.qualified_name.clone(),
            InterfaceInstance::Topology(t) => t.qualified_name.clone(),
            // A bound parameter is not a global definition, so its name is the
            // identifier it introduces in the expansion
            InterfaceInstance::TemplateArg(arg) => arg.param_def.name.data.clone(),
        }
    }

    /// Gets the unqualified name of the interface instance
    pub fn unqualified_name(&self) -> String {
        match self {
            InterfaceInstance::Component(ci) => ci.get_unqualified_name().to_string(),
            InterfaceInstance::Topology(t) => t.symbol.name().data.clone(),
            InterfaceInstance::TemplateArg(arg) => arg.param_def.name.data.clone(),
        }
    }

    /// Gets the location of the interface instance
    pub fn get_loc(&self) -> Span {
        match self {
            InterfaceInstance::Component(ci) => ci.get_loc(),
            InterfaceInstance::Topology(t) => t.symbol.node().span(),
            InterfaceInstance::TemplateArg(arg) => arg.param_def.interface.span(),
        }
    }

    /// Get the full port interface of this instance.
    ///
    /// A topology instance holds the topology's symbol and looks its port
    /// interface up in [`Analysis::topology_map`], so the result is `None` only
    /// for a topology that is not in that map.
    pub fn get_interface<'a>(&'a self, a: &'a Analysis) -> Option<&'a PortInterface> {
        match self {
            InterfaceInstance::Component(ci) => ci.get_interface(a),
            InterfaceInstance::Topology(t) => {
                a.topology_map.get(&t.symbol).map(|top| &top.port_interface)
            }
            InterfaceInstance::TemplateArg(arg) => Some(&arg.declared_interface.port_interface),
        }
    }

    /// Get the underlying component instance from this interface instance,
    /// `None` if it's a topology
    pub fn get_component_instance_opt(&self) -> Option<&ComponentInstance> {
        match self {
            InterfaceInstance::Component(ci) => Some(ci),
            InterfaceInstance::Topology(_) => None,
            InterfaceInstance::TemplateArg(arg) => arg.instance.get_component_instance_opt(),
        }
    }

    /// Get a port instance given the name of the port instance, erroring at
    /// the name's own span if it is not present
    pub fn lookup_port_instance(&self, a: &Analysis, name: &Ident) -> SemanticResult<PortInstance> {
        self.require_port_instance(a, &name.data, name.span())
    }

    /// Look up the resolved topology this instance refers to, if any.
    /// Rust-only: a topology instance holds the topology's symbol, and the
    /// topology lives in the analysis.
    ///
    /// Returns `None` for component instances or if the topology is not (yet)
    /// resolved in the analysis.
    pub fn as_topology<'a>(&self, a: &'a Analysis) -> Option<&'a Topology> {
        match self {
            InterfaceInstance::Component(_) => None,
            InterfaceInstance::Topology(t) => a.topology_map.get(&t.symbol),
            InterfaceInstance::TemplateArg(arg) => arg.instance.as_topology(a),
        }
    }

    /// Look up a port instance by name in this interface instance. Rust-only:
    /// a lookup by plain name that reports no error.
    pub fn get_port_instance(&self, a: &Analysis, name: &str) -> Option<PortInstance> {
        let interface = self
            .get_interface(a)
            .expect("interface instance references a resolved component or topology");
        interface.get_port_instance(name)
    }

    /// The name to report this instance under when it has no port instance of a
    /// given name. A bound parameter is reported against the port interface
    /// declared in the parameter, because that is the only interface visible
    /// inside the expansion. Rust-only.
    fn port_instance_error_name(&self) -> String {
        match self {
            InterfaceInstance::TemplateArg(arg) => {
                arg.declared_interface.get_unqualified_name().to_string()
            }
            _ => self.unqualified_name(),
        }
    }

    /// Look up a port instance by name, erroring at `loc` if it is not
    /// present on this interface instance. Rust-only: the lookup of
    /// [`InterfaceInstance::lookup_port_instance`] for a name that carries no
    /// span of its own.
    pub fn require_port_instance(
        &self,
        a: &Analysis,
        name: &str,
        loc: Span,
    ) -> SemanticResult<PortInstance> {
        self.get_port_instance(a, name)
            .ok_or_else(|| SemanticError::InvalidPortInstanceId {
                loc,
                port_name: name.to_string(),
                instance_type: self
                    .get_interface(a)
                    .expect("interface instance references a resolved component or topology")
                    .instance_type
                    .clone(),
                interface_name: self.port_instance_error_name(),
            })
    }
}

impl PartialEq for InterfaceInstance {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for InterfaceInstance {}
impl PartialOrd for InterfaceInstance {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
/// Ordering on interface instances.
impl Ord for InterfaceInstance {
    fn cmp(&self, other: &Self) -> Ordering {
        self.qualified_name()
            .cmp(&other.qualified_name())
            .then_with(|| self.kind_ordinal().cmp(&other.kind_ordinal()))
            .then_with(|| match (self, other) {
                // Two bound template parameters with the same name are the same
                // instance only if they name the same parameter of the same
                // template and are bound to the same argument
                (InterfaceInstance::TemplateArg(a1), InterfaceInstance::TemplateArg(a2)) => {
                    a1.instance.cmp(&a2.instance).then_with(|| {
                        a1.param_def
                            .interface
                            .id()
                            .cmp(&a2.param_def.interface.id())
                    })
                }
                // Component instance and topology names are unique in a model,
                // so equal names mean equal instances
                _ => Ordering::Equal,
            })
    }
}

impl InterfaceInstance {
    /// An ordinal for each kind of instance, used to break ties between
    /// instances of different kinds that have the same qualified name
    fn kind_ordinal(&self) -> u8 {
        match self {
            InterfaceInstance::Component(_) => 0,
            InterfaceInstance::Topology(_) => 1,
            InterfaceInstance::TemplateArg(_) => 2,
        }
    }
}

/// An imported topology used as an interface instance.
///
/// It stores the topology symbol and qualified name. The resolved [`Topology`]
/// is looked up in [`Analysis::topology_map`] when its port interface is
/// needed, so cloning a [`Connection`](crate::semantics::Connection) does not
/// copy the imported topologies.
#[derive(Debug, Clone)]
pub struct TopologyInstance {
    /// The topology symbol, used to look up the resolved `Topology`.
    pub symbol: Symbol,
    /// The fully qualified name of the topology
    pub qualified_name: String,
}

impl TopologyInstance {
    /// Builds a port instance identifier for a top-level port on this
    /// topology instance, by name. Errors are annotated at this instance's
    /// own location, since a plain name carries no span of its own. Rust-only,
    /// for the Python bindings.
    pub fn get_port_instance_identifier(
        &self,
        a: &Analysis,
        name: &str,
    ) -> SemanticResult<PortInstanceIdentifier> {
        let interface_instance = InterfaceInstance::Topology(self.clone());
        let loc = interface_instance.get_loc();
        let port_instance = interface_instance.require_port_instance(a, name, loc)?;
        Ok(PortInstanceIdentifier {
            interface_instance,
            port_instance,
        })
    }
}

/// A bound port interface instance template parameter used as an interface
/// instance.
///
/// The port interface is the one declared in the parameter, not the one of the
/// instance bound to it: inside the expansion only the declared interface is
/// visible. `instance` is the interface instance the argument refers to, which
/// may itself be a bound parameter.
#[derive(Debug, Clone)]
pub struct TemplateArgInstance {
    /// The parameter this argument is bound to
    pub param_def: Arc<ast::TemplateParamInterface>,
    /// The interface declared in the parameter, whose port interface is the one
    /// [`InterfaceInstance::get_interface`] reports for this instance
    pub declared_interface: Interface,
    /// The interface instance the argument refers to
    pub instance: Box<InterfaceInstance>,
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

    pub fn from_template_arg(
        param_def: Arc<ast::TemplateParamInterface>,
        declared_interface: Interface,
        instance: InterfaceInstance,
    ) -> InterfaceInstance {
        InterfaceInstance::TemplateArg(TemplateArgInstance {
            param_def,
            declared_interface,
            instance: Box::new(instance),
        })
    }
}
