//! Resolve a topology definition.
//!
//! The resolution proceeds in the following stages:
//!
//! 1. Resolve the directly declared instances.
//! 2. Resolve the topology port interface.
//! 3. Resolve the partially numbered topology (imports, patterns,
//!    interface-to-component resolution).
//! 4. Resolve the port numbers.
//! 5. Resolve the unconnected ports.
//! 6. Check that the topology implements its declared interfaces.

mod check_topology_interface;
mod general_port_numbering;
mod matched_port_numbering;
mod pattern_resolver;
mod port_numbering_state;
mod resolve_partially_numbered;
mod resolve_port_numbers;
mod resolve_topology_instances;
mod resolve_topology_port_interface;
mod resolve_unconnected_ports;

use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::{InterfaceInstance, PortInstance, PortInstanceIdentifier, Topology};

/// Fold over every port instance identifier of the topology's component
/// instances, in a deterministic order. Topology-alias instances are excluded:
/// their connections were already rewritten to the underlying component
/// instances.
pub(crate) fn for_each_port(a: &Analysis, t: &Topology) -> Vec<PortInstanceIdentifier> {
    let mut result = Vec::new();
    for ci in t.component_instance_map().keys() {
        let Some(port_interface) = ci.get_interface(a) else {
            continue;
        };
        let mut ports: Vec<(&String, &PortInstance)> = port_interface.port_map.iter().collect();
        ports.sort_by(|x, y| x.0.cmp(y.0));
        for (_, pi) in ports {
            result.push(PortInstanceIdentifier {
                interface_instance: InterfaceInstance::from_component_instance(ci.clone()),
                port_instance: pi.clone(),
            });
        }
    }
    result
}

/// Resolve this topology definition.
pub fn resolve(a: &Analysis, t: &mut Topology) -> SemanticResult {
    resolve_topology_instances::resolve(a, t);
    resolve_topology_port_interface::resolve(a, t)?;
    resolve_partially_numbered::resolve(a, t)?;
    resolve_port_numbers::resolve(a, t)?;
    resolve_unconnected_ports::resolve(a, t);

    // Check the topologies interface against the `implements` clause
    check_topology_interface::check(a, t)?;
    Ok(())
}
