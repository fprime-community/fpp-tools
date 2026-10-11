//! Resolve unconnected ports

use crate::Analysis;
use crate::semantics::{InterfaceInstance, PortInstanceIdentifier, Topology};

/// Compute the unconnected ports of t
pub fn resolve(a: &Analysis, t: &mut Topology) {
    // Fold over instances and ports
    for ci in t.component_instance_map().into_keys() {
        for pi in ci.component(a).port_interface.port_map.values() {
            let pii = PortInstanceIdentifier {
                interface_instance: InterfaceInstance::from_component_instance(ci.clone()),
                port_instance: pi.clone(),
            };
            let direction = pi.get_direction();
            let n = t.get_connections_at(&pii).len();
            if let (Some(_), 0) = (direction, n) {
                t.unconnected_port_set.insert(pii);
            }
        }
    }
}
