use crate::run_test;

#[test]
fn duplicate_output_connection() {
    run_test("port_numbering/duplicate_output_connection")
}

#[test]
fn no_port_available_for_matched_numbering() {
    run_test("port_numbering/no_port_available_for_matched_numbering")
}

#[test]
fn implicit_duplicate_connection_at_matched_output_port() {
    run_test("port_numbering/implicit_duplicate_connection_at_matched_output_port")
}

#[test]
fn ok() {
    run_test("port_numbering/ok")
}

#[test]
fn mismatched_port_numbers() {
    run_test("port_numbering/mismatched_port_numbers")
}

#[test]
fn too_many_output_ports() {
    run_test("port_numbering/too_many_output_ports")
}

#[test]
fn duplicate_matched_connection() {
    run_test("port_numbering/duplicate_matched_connection")
}

#[test]
fn negative_port_number() {
    run_test("port_numbering/negative_port_number")
}

#[test]
fn implicit_duplicate_connection_at_matched_input_port() {
    run_test("port_numbering/implicit_duplicate_connection_at_matched_input_port")
}

#[test]
fn duplicate_connection_at_matched_port() {
    run_test("port_numbering/duplicate_connection_at_matched_port")
}

#[test]
fn missing_connection() {
    run_test("port_numbering/missing_connection")
}

/// Two component instance definitions that share a qualified name, in a
/// topology that also uses matched port numbering.
///
/// `ComponentInstance` compares, orders and hashes by qualified name, so
/// `MatchedPortNumbering`'s instance-to-connection map would merge two
/// same-named instances where Scala's structurally-keyed
/// `Map[ComponentInstance, Connection]` keeps them apart. That is unreachable:
/// the symbol table rejects the second definition and keeps the first, so every
/// use of `c2` — the topology instance spec and both of its connections —
/// resolves to one symbol, hence one instance. The only diagnostic is the
/// redefinition; matched numbering still sees `c2` and `c3` as the two distinct
/// remote instances of `c1.pOut`/`c1.pIn` and assigns them separate port
/// numbers.
#[test]
fn duplicate_instance_name() {
    run_test("port_numbering/duplicate_instance_name")
}

/// Matched port numbering of connections written through topology port aliases.
///
/// `c1`'s matched ports are reached from topology `T` through the aliases
/// `A.aOut`/`A.aIn`. `MatchedPortNumbering` keys its instance-to-connection maps
/// by remote *component* instance, which works only because every endpoint was
/// rewritten from the imported topology down to the component instance owning
/// the port before numbering ran. Were an endpoint left pointing at the imported
/// topology, both matched connections would drop out of the maps and this
/// diagnostic would silently disappear.
#[test]
fn matched_through_alias() {
    run_test("port_numbering/matched_through_alias")
}
