use super::*;

/// Parse a whole file as a sequence of members of kind `member`.
fn entry(
    p: &mut Parser,
    member: impl Fn(&mut Parser),
    delim: SyntaxKind,
    annotated: bool,
    expected_error_msg: &'static str,
) {
    let m = p.start();
    element_sequence(p, EOF, member, delim, annotated, expected_error_msg);
    m.complete(p, ROOT);
}

pub(crate) fn module_entry(p: &mut Parser) {
    entry(
        p,
        module::module_member,
        SEMI,
        true,
        "module member expected",
    );
}

pub(crate) fn component_entry(p: &mut Parser) {
    entry(
        p,
        component::component_member,
        SEMI,
        true,
        "component member expected",
    );
}

pub(crate) fn topology_entry(p: &mut Parser) {
    entry(
        p,
        topology::topology_member,
        SEMI,
        true,
        "topology member expected",
    );
}

pub(crate) fn tlm_packet_entry(p: &mut Parser) {
    entry(
        p,
        topology::tlm_packet_member,
        COMMA,
        false,
        "telemetry packet member expected",
    );
}

pub(crate) fn tlm_packet_set_entry(p: &mut Parser) {
    entry(
        p,
        topology::tlm_packet_set_member,
        COMMA,
        true,
        "telemetry packet set member expected",
    );
}

pub(crate) fn state_machine_entry(p: &mut Parser) {
    entry(
        p,
        state_machine::state_machine_member,
        SEMI,
        true,
        "state machine member expected",
    );
}

pub(crate) fn state_entry(p: &mut Parser) {
    entry(
        p,
        state_machine::state_member,
        SEMI,
        true,
        "state member expected",
    );
}
