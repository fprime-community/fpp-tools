//! Resolve port numbers

use super::{general_port_numbering, matched_port_numbering};
use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{Connection, PortInstanceIdentifier, Topology};
use rustc_hash::FxHashMap as HashMap;
use std::collections::BTreeSet;

/// Check output ports for t
fn check_output_ports(t: &Topology) -> SemanticResult {
    for (pii, s) in &t.output_connection_map {
        check_output_size_bounds(pii, s)?;
        check_duplicate_output_connections(s)?;
    }
    Ok(())
}

/// Check that there are no duplicate port numbers at any output
/// ports.
fn check_duplicate_output_connections(connections: &BTreeSet<Connection>) -> SemanticResult {
    let mut port_num_map: HashMap<i128, Connection> = HashMap::default();
    for c in connections {
        if let Some(port_num) = c.from.port_number {
            match port_num_map.get(&port_num) {
                Some(prev_c) => {
                    let loc = c.from.loc;
                    let prev_loc = prev_c.from.loc;
                    return Err(SemanticError::DuplicateOutputConnection {
                        loc,
                        port_num,
                        prev_loc,
                    });
                }
                None => {
                    port_num_map.insert(port_num, c.clone());
                }
            }
        }
    }
    Ok(())
}

/// Check the bounds on the number of output connections
fn check_output_size_bounds(
    pii: &PortInstanceIdentifier,
    connections: &BTreeSet<Connection>,
) -> SemanticResult {
    let pi = &pii.port_instance;
    let array_size = pi.get_array_size();
    let num_ports = connections.len() as i128;
    if num_ports <= array_size {
        Ok(())
    } else {
        let loc = pi.get_loc();
        let instance_loc = pii.interface_instance.get_loc();
        Err(SemanticError::TooManyOutputPorts {
            loc,
            num_ports,
            array_size,
            instance_loc,
        })
    }
}

/// Fill in the port numbers for this topology
pub fn resolve(a: &Analysis, t: &mut Topology) -> SemanticResult {
    check_output_ports(t)?;
    matched_port_numbering::apply(a, t)?;
    general_port_numbering::apply(a, t);
    Ok(())
}
