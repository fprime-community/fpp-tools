use crate::run_test;

#[test]
fn implements_port_missing() {
    run_test("top_ports/implements_port_missing")
}

#[test]
fn implements_port_mismatch_1() {
    run_test("top_ports/implements_port_mismatch_1")
}

#[test]
fn nested() {
    run_test("top_ports/nested")
}

#[test]
fn unmatched_types() {
    run_test("top_ports/unmatched_types")
}

#[test]
fn implements_port_mismatch_2() {
    run_test("top_ports/implements_port_mismatch_2")
}

#[test]
fn implements() {
    run_test("top_ports/implements")
}

#[test]
fn basic() {
    run_test("top_ports/basic")
}

#[test]
fn out_to_out() {
    run_test("top_ports/out_to_out")
}

#[test]
fn internal_port() {
    run_test("top_ports/internal_port")
}

#[test]
fn interface_instance_not_member() {
    run_test("top_ports/interface_instance_not_member")
}

#[test]
fn top_to_top() {
    run_test("top_ports/top_to_top")
}

#[test]
fn unmatched_through_alias() {
    run_test("top_ports/unmatched_through_alias")
}

/// With several ports missing, the one reported is the first one added to the
/// interface.
///
/// `PortInterface::implements` reports only its first offending port, so which
/// diagnostic comes out depends on the order it walks the interface's port map.
/// Scala walks `other.portMap.toList` over an immutable `Map`, which preserves
/// insertion order up to four entries, and `fpp-check` anchors this input at
/// `sss`. Iterating the `FxHashMap` directly picks `qqq` instead, so this
/// fixture pins the ordering rather than the hash layout. This interface imports
/// nothing, so its insertion order is also its source order.
#[test]
fn implements_first_missing_port() {
    run_test("top_ports/implements_first_missing_port")
}

/// The port reported is the first one *added* to the interface, which is not the
/// first one defined: an interface's own ports are added as its members are
/// visited, and the ports it imports only afterwards, when it is resolved.
///
/// Here `I` imports two ports defined above it and declares two of its own, so
/// insertion order is `b1 b2 a1 a2` while source order is `a1 a2 b1 b2`.
/// `fpp-check` anchors the input at `b1`, confirming insertion order; ordering by
/// source position would report `a1`.
#[test]
fn implements_missing_port_import_order() {
    run_test("top_ports/implements_missing_port_import_order")
}

/// Past four entries, Scala's immutable `Map` stops preserving insertion order
/// and iterates in an order derived from the key hashes, so no port ordering
/// reproduces it. This interface has five ports, and `fpp-check` anchors the
/// input at `p3` while we report `p5`; neither answer is more correct than the
/// other. The fixture pins that our answer stays deterministic (the first port
/// added) instead of following `FxHashMap` layout.
#[test]
fn implements_missing_port_beyond_four() {
    run_test("top_ports/implements_missing_port_beyond_four")
}
