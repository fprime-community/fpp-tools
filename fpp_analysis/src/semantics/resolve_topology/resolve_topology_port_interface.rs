use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::{PortInstanceIdentifier, Topology};

/// Resolve a topology's port interface
pub fn resolve(a: &Analysis, t: &mut Topology) -> SemanticResult {
    let ports = t.ports.clone();
    for a_node in ports {
        if let Some(instance) = PortInstanceIdentifier::from_node(a, &a_node.underlying_port)? {
            t.add_port(a_node, instance)?;
        }
    }
    Ok(())
}
