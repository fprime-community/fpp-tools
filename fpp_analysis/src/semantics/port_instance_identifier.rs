use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::{InterfaceInstance, PortInstance};
use fpp_ast::{self as ast, AstNode};
use std::cmp::Ordering;

/// An FPP port instance identifier
#[derive(Debug, Clone)]
pub struct PortInstanceIdentifier {
    /// The interface instance
    pub interface_instance: InterfaceInstance,
    /// The port instance
    pub port_instance: PortInstance,
}

impl std::fmt::Display for PortInstanceIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.qualified_name())
    }
}

impl PartialEq for PortInstanceIdentifier {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for PortInstanceIdentifier {}
impl PartialOrd for PortInstanceIdentifier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for PortInstanceIdentifier {
    /// Compare two port instance identifiers
    fn cmp(&self, other: &Self) -> Ordering {
        self.qualified_name()
            .cmp(&other.qualified_name())
            .then_with(|| self.interface_instance.cmp(&other.interface_instance))
            .then_with(|| {
                self.port_instance
                    .get_node_id()
                    .cmp(&other.port_instance.get_node_id())
            })
    }
}

impl PortInstanceIdentifier {
    /// Gets the qualified name
    pub fn qualified_name(&self) -> String {
        format!(
            "{}.{}",
            self.interface_instance.qualified_name(),
            self.port_instance.get_unqualified_name()
        )
    }

    /// Gets the unqualified name
    pub fn get_unqualified_name(&self) -> String {
        format!(
            "{}.{}",
            self.interface_instance.unqualified_name(),
            self.port_instance.get_unqualified_name()
        )
    }

    /// Creates a port instance identifier from an AST node. Returns `None` if
    /// the interface instance use is unresolved, or names an instance whose
    /// definition failed to check (both already reported).
    pub fn from_node(
        a: &Analysis,
        node: &ast::PortInstanceIdentifier,
    ) -> SemanticResult<Option<PortInstanceIdentifier>> {
        let Some(interface_instance) = a.get_interface_instance(node.interface_instance.id())?
        else {
            return Ok(None);
        };
        let port_instance = interface_instance.lookup_port_instance(a, &node.port_name)?;
        Ok(Some(PortInstanceIdentifier {
            interface_instance,
            port_instance,
        }))
    }
}
