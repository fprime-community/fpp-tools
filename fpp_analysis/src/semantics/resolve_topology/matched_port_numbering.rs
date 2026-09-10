use super::port_numbering_state::PortNumberingState;
use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    ComponentInstance, Connection, InterfaceInstance, PortInstance, PortInstanceIdentifier,
    PortMatching, Topology,
};
use fpp_core::Span;
use rustc_hash::FxHashMap as HashMap;
use std::collections::BTreeSet;

// Apply matched port numbering

// A map from component instances to connections for tracking
// matching pairs of connections
type InstanceConnectionMap = HashMap<ComponentInstance, Connection>;

// A map from port numbers to connections for tracking port
// assignments
type PortConnectionMap = HashMap<i128, Connection>;

// State for matched port numbering
struct State {
    // The port instance for port 1
    pi1: PortInstance,
    // The map from component instances to connections for port 1
    icm1: InstanceConnectionMap,
    // The map from port numbers to connections for port 1
    pcm1: PortConnectionMap,
    // The port instance for port 2
    pi2: PortInstance,
    // The map from component instances to connections for port 2
    icm2: InstanceConnectionMap,
    // The map from port numbers to connections for port 2
    pcm2: PortConnectionMap,
    // Port numbering state
    numbering: PortNumberingState,
}

impl State {
    // Marks the specified port number as used and generates a new one
    fn use_port_number(&mut self, n: i128) {
        self.numbering = self.numbering.use_port_number(n);
    }

    // Gets the next port number and updates the port numbering state
    fn get_port_number(&mut self) -> i128 {
        let (s, n) = self.numbering.get_port_number();
        self.numbering = s;
        n
    }

    // Adds a mapping to pcm1 and updates the port numbering state
    fn update_port_connection_map1(&mut self, n: i128, c: Connection) {
        self.use_port_number(n);
        self.pcm1.insert(n, c);
    }

    // Adds a mapping to pcm2 and updates the port numbering state
    fn update_port_connection_map2(&mut self, n: i128, c: Connection) {
        self.use_port_number(n);
        self.pcm2.insert(n, c);
    }

    fn initial(
        pi1: PortInstance,
        icm1: InstanceConnectionMap,
        pcm1: PortConnectionMap,
        pi2: PortInstance,
        icm2: InstanceConnectionMap,
        pcm2: PortConnectionMap,
    ) -> State {
        // Compute the used port numbers
        let mut used: BTreeSet<i128> = BTreeSet::new();
        used.extend(pcm1.keys().copied());
        used.extend(pcm2.keys().copied());
        State {
            pi1,
            icm1,
            pcm1,
            pi2,
            icm2,
            pcm2,
            numbering: PortNumberingState::initial(used),
        }
    }
}

// Number a connection pair
fn number_connection_pair(
    t: &mut Topology,
    state: &mut State,
    matching_loc: Span,
    c1: &Connection,
    c2: &Connection,
) -> SemanticResult {
    let n1_opt = t.get_port_number(&state.pi1, c1);
    let n2_opt = t.get_port_number(&state.pi2, c2);
    match (n1_opt, n2_opt) {
        (Some(n1), Some(n2)) => {
            // Both ports have a number
            if n1 == n2 {
                // Numbers match: OK, nothing to do
                Ok(())
            } else {
                // Numbers don't match: error
                let p1_loc = c1.get_this_endpoint(&state.pi1).loc;
                let p2_loc = c2.get_this_endpoint(&state.pi2).loc;
                Err(SemanticError::MismatchedPortNumbers {
                    p1_loc,
                    p1_number: n1,
                    p2_loc,
                    p2_number: n2,
                    matching_loc,
                })
            }
        }
        (Some(n), None) => {
            // Only pi1 has a number
            match state.pcm2.get(&n) {
                Some(prev_c) => {
                    // Number is already assigned at pi2: error
                    Err(SemanticError::ImplicitDuplicateConnectionAtMatchedPort {
                        loc: c2.get_loc(),
                        port: state.pi2.to_string(),
                        port_num: n,
                        implying_loc: c1.get_loc(),
                        matching_loc,
                        prev_loc: prev_c.get_loc(),
                    })
                }
                None => {
                    // Assign the number to c2 at pi2 and update the state
                    t.assign_port_number(&state.pi2, c2, n);
                    state.update_port_connection_map2(n, c2.clone());
                    Ok(())
                }
            }
        }
        (None, Some(n)) => {
            // Only pi2 has a number
            match state.pcm1.get(&n) {
                Some(prev_c) => {
                    // Number is already assigned at pi1: error
                    Err(SemanticError::ImplicitDuplicateConnectionAtMatchedPort {
                        loc: c1.get_loc(),
                        port: state.pi1.to_string(),
                        port_num: n,
                        implying_loc: c2.get_loc(),
                        matching_loc,
                        prev_loc: prev_c.get_loc(),
                    })
                }
                None => {
                    // Assign the number to c1 at pi1 and update the state
                    t.assign_port_number(&state.pi1, c1, n);
                    state.update_port_connection_map1(n, c1.clone());
                    Ok(())
                }
            }
        }
        (None, None) => {
            // Neither port has a number; get a new one
            let n = state.get_port_number();
            if n >= state.pi1.get_array_size() {
                // Port number is out of range: error
                Err(SemanticError::NoPortAvailableForMatchedNumbering {
                    loc1: c1.get_loc(),
                    loc2: c2.get_loc(),
                    matching_loc,
                })
            } else {
                // Assign the number to both sides and update the state
                t.assign_port_number(&state.pi1, c1, n);
                t.assign_port_number(&state.pi2, c2, n);
                state.update_port_connection_map1(n, c1.clone());
                state.update_port_connection_map2(n, c2.clone());
                Ok(())
            }
        }
    }
}

// For each pair of connections (c1, c2), check that numbers
// match and/or assign numbers
fn assign_numbers(t: &mut Topology, state: &mut State, matching_loc: Span) -> SemanticResult {
    // Take icm1 out of the state, so that numbering it needs no copy. Nothing
    // reads it again: the state is dropped once the matching is numbered.
    let mut list1: Vec<(ComponentInstance, Connection)> =
        std::mem::take(&mut state.icm1).into_iter().collect();
    list1.sort_by(|x, y| x.1.cmp(&y.1));
    for (ci, c1) in list1 {
        let c2 = state
            .icm2
            .get(&ci)
            .expect("matching instance present in icm2")
            .clone();
        number_connection_pair(t, state, matching_loc, &c1, &c2)?;
    }
    Ok(())
}

/// Apply matched numbering
pub fn apply(a: &Analysis, t: &mut Topology) -> SemanticResult {
    // Fold over instances and matchings
    for (ci, _) in t.component_instance_map() {
        let Some(comp) = a.component_map.get(&ci.component_symbol) else {
            continue;
        };
        let matchings = comp.port_matching_list.clone();
        for pm in &matchings {
            handle_port_matching(t, &ci, pm)?;
        }
    }
    Ok(())
}

// Check for missing connections
fn check_for_missing_connections(
    matching_loc: Span,
    icm1: &InstanceConnectionMap,
    icm2: &InstanceConnectionMap,
) -> SemanticResult {
    // Ensure that icm2 contains everything in icm1
    fn helper(
        matching_loc: Span,
        icm1: &InstanceConnectionMap,
        icm2: &InstanceConnectionMap,
    ) -> SemanticResult {
        for (ci, c) in icm1 {
            if !icm2.contains_key(ci) {
                return Err(SemanticError::MissingConnection {
                    loc: c.get_loc(),
                    matching_loc,
                });
            }
        }
        Ok(())
    }
    // Ensure that the two sets of keys match
    if icm1.len() >= icm2.len() {
        helper(matching_loc, icm1, icm2)
    } else {
        helper(matching_loc, icm2, icm1)
    }
}

// Handle one port matching
fn handle_port_matching(
    t: &mut Topology,
    ci: &ComponentInstance,
    port_matching: &PortMatching,
) -> SemanticResult {
    // A matching is between two general ports; the numbering machinery below
    // works over any kind of port instance.
    let pi1 = PortInstance::General(port_matching.instance1.clone());
    let pi2 = PortInstance::General(port_matching.instance2.clone());
    let loc = port_matching.get_loc();

    let pcm1 = compute_port_connection_map(t, ci, &pi1, loc)?;
    let pcm2 = compute_port_connection_map(t, ci, &pi2, loc)?;
    let icm1 = compute_instance_connection_map(t, ci, &pi1, loc)?;
    let icm2 = compute_instance_connection_map(t, ci, &pi2, loc)?;
    check_for_missing_connections(loc, &icm1, &icm2)?;

    let mut state = State::initial(pi1, icm1, pcm1, pi2, icm2, pcm2);
    assign_numbers(t, &mut state, loc)
}

// Map remote component instances to connections at pi
fn compute_instance_connection_map(
    t: &Topology,
    ci: &ComponentInstance,
    pi: &PortInstance,
    matching_loc: Span,
) -> SemanticResult<InstanceConnectionMap> {
    let pii = PortInstanceIdentifier {
        interface_instance: InterfaceInstance::from_component_instance(ci.clone()),
        port_instance: pi.clone(),
    };
    let cs = t.get_connections_at(&pii);
    let mut m: InstanceConnectionMap = HashMap::default();
    for c in cs {
        if c.is_unmatched {
            continue;
        }
        let pii_remote = &c.get_other_endpoint(pi).port;
        // Every endpoint of a connection that reaches port numbering refers to a
        // component instance (after flattenning)
        debug_assert!(
            pii_remote
                .interface_instance
                .get_component_instance_opt()
                .is_some(),
            "endpoint {} should refer to a component instance, not to an imported topology",
            pii_remote.qualified_name()
        );
        let Some(ci_remote) = pii_remote.interface_instance.get_component_instance_opt() else {
            continue;
        };
        let ci_remote = ci_remote.clone();
        match m.get(&ci_remote) {
            Some(c_prev) => {
                return Err(SemanticError::DuplicateMatchedConnection {
                    loc: c.get_loc(),
                    prev_loc: c_prev.get_loc(),
                    matching_loc,
                });
            }
            None => {
                m.insert(ci_remote, c);
            }
        }
    }
    Ok(m)
}

// Map port numbers to connections at pi
// While computing the map, enforce the rule against duplicate connections
fn compute_port_connection_map(
    t: &Topology,
    ci: &ComponentInstance,
    pi: &PortInstance,
    matching_loc: Span,
) -> SemanticResult<PortConnectionMap> {
    let pii = PortInstanceIdentifier {
        interface_instance: InterfaceInstance::from_component_instance(ci.clone()),
        port_instance: pi.clone(),
    };
    let cs = t.get_connections_at(&pii);
    let mut m: PortConnectionMap = HashMap::default();
    for c in cs {
        if let Some(n) = t.get_port_number(pi, &c) {
            match m.get(&n) {
                Some(prev_c) => {
                    return Err(SemanticError::DuplicateConnectionAtMatchedPort {
                        loc: c.get_loc(),
                        port: pi.to_string(),
                        port_num: n,
                        prev_loc: prev_c.get_loc(),
                        matching_loc,
                    });
                }
                None => {
                    m.insert(n, c);
                }
            }
        }
    }
    Ok(m)
}
