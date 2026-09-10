use crate::run_test;

#[test]
fn empty_ok() {
    run_test("interface/empty_ok")
}

#[test]
fn ok() {
    run_test("interface/ok")
}

#[test]
fn duplicate_import() {
    run_test("interface/duplicate_import")
}

#[test]
fn duplicate_name() {
    run_test("interface/duplicate_name")
}

#[test]
fn cycles() {
    run_test("interface/cycles")
}

#[test]
fn conflict_name() {
    run_test("interface/conflict_name")
}

#[test]
fn async_port_in_passive() {
    run_test("interface/async_port_in_passive")
}

/// Two interfaces import the same interface, and a fourth imports both: the
/// port arrives twice. The reported import is the second one in source order,
/// because the imports are merged in source order.
#[test]
fn diamond_import() {
    run_test("interface/diamond_import")
}

/// An interface whose ports conflict with two of its imports reports only the
/// first offending import in source order, like Scala's `Result.foldLeft` over
/// `ResolveInterface.resolve`.
#[test]
fn import_conflict_order() {
    run_test("interface/import_conflict_order")
}

/// A cycle of interface imports is caught as a use-def cycle, before interface
/// resolution can recur through it.
#[test]
fn import_cycle() {
    run_test("interface/import_cycle")
}

#[test]
fn undefined_import() {
    run_test("interface/undefined_import")
}

/// Importing an interface that failed to resolve reports only that interface's
/// own error.
#[test]
fn import_of_broken_interface() {
    run_test("interface/import_of_broken_interface")
}

/// Merging an import stops at the first port that collides, so which collision
/// is reported depends on the order the imported interface's ports are walked.
/// They are walked in the order they were added, as Scala's fold over
/// `interface.portInterface.portMap.values.toList` does: every one of `A`'s four
/// ports collides, and `fpp-check` anchors the input at `q1`, the first one `A`
/// declares.
#[test]
fn import_duplicate_port_order() {
    run_test("interface/import_duplicate_port_order")
}

/// The order an imported interface's ports are walked in is the order they were
/// added to it, not the order they were defined: `A` declares `zzz` and imports
/// `aaa` from an interface above it, so it holds them in the order `zzz aaa`
/// while they are defined in the order `aaa zzz`. `fpp-check` anchors the input
/// at `zzz`; ordering by source position would report `aaa`.
#[test]
fn import_duplicate_port_import_order() {
    run_test("interface/import_duplicate_port_import_order")
}

/// A collision on the special port map is ordered the same way: `A`'s ports have
/// names of their own, so the duplicate is of a special *kind*, and which kind is
/// reported follows the order `A` holds its ports in. `fpp-check` anchors the
/// input at `command recv`, from `A`'s first port.
#[test]
fn import_duplicate_special_port_order() {
    run_test("interface/import_duplicate_special_port_order")
}

/// An import that fails does not discard the imports that already merged. `Mid`
/// imports the same port from `A` and from `B`; the second import fails, but
/// `Mid` keeps the port it got from the first, so `Outer`, the component that
/// imports it, and the connection to `c.x` all still resolve. Only the one
/// import failure is reported, exactly as `fpp-check` reports it.
#[test]
fn failed_import_keeps_merged_ports() {
    run_test("interface/failed_import_keeps_merged_ports")
}
