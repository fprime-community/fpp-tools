use super::for_each_port;
use super::port_numbering_state::PortNumberingState;
use crate::Analysis;
use crate::semantics::{Direction, PortInstanceIdentifier, Topology};

// Apply general port numbering

// Number an input port array
fn number_input_port_array(t: &mut Topology, pii: &PortInstanceIdentifier) {
    let pi = &pii.port_instance;
    for c in t.get_connections_to(pii) {
        match t.get_port_number(pi, &c) {
            Some(_n) => {}
            None => t.assign_port_number(pi, &c, 0),
        }
    }
}

// Number an output port array
fn number_output_port_array(t: &mut Topology, pii: &PortInstanceIdentifier) {
    let pi = &pii.port_instance;
    let cs = t.get_connections_from(pii);
    let used_port_numbers = t.get_used_port_numbers(pi, &cs);
    let mut state = PortNumberingState::initial(used_port_numbers);
    for c in cs {
        match t.get_port_number(pi, &c) {
            Some(_n) => {}
            None => {
                let (s1, n) = state.get_port_number();
                state = s1;
                t.assign_port_number(pi, &c, n);
            }
        }
    }
}

/// Apply general numbering
pub fn apply(a: &Analysis, t: &mut Topology) {
    // Fold over instances and ports
    for pii in for_each_port(a, t) {
        match pii.port_instance.get_direction() {
            Some(Direction::Input) => number_input_port_array(t, &pii),
            Some(Direction::Output) => number_output_port_array(t, &pii),
            None => {}
        }
    }
}
