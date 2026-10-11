//! Regression tests for recursive `.fppi` include formatting.

use fpp_format::{FormatError, FormatOptions, format_file_recursive};
use fpp_lsp_parser::TopEntryPoint;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A self-cleaning unique temp directory.
struct TmpDir(PathBuf);

impl TmpDir {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("fpp_format_inc_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&dir).unwrap();
        TmpDir(dir)
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, contents).unwrap();
        path
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.0.join(name)).unwrap()
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fmt(path: &Path) -> Result<Vec<fpp_format::FormattedUnit>, FormatError> {
    format_file_recursive(path, TopEntryPoint::Module, FormatOptions::default())
}

#[test]
fn recurses_and_derives_entrypoints() {
    let dir = TmpDir::new();
    // Root includes a module fragment; also nests a component and topology
    // whose bodies include fragments that must be parsed with the matching
    // entrypoints (component members / topology members are not valid at
    // module scope).
    dir.write(
        "root.fpp",
        "module M {\n\
         include \"consts.fppi\"\n\
         active component C {\n\
         include \"ports.fppi\"\n\
         }\n\
         topology T {\n\
         include \"conns.fppi\"\n\
         }\n\
         }\n",
    );
    // Module fragment: unaligned constants that should get aligned.
    dir.write("consts.fppi", "constant a = 1\nconstant bbbb = 2\n");
    // Component fragment: component members (invalid at module scope).
    dir.write(
        "ports.fppi",
        "sync input port pIn: P\noutput port pOut: P\n",
    );
    // Topology fragment: connections (invalid at module scope).
    dir.write("conns.fppi", "connections C {\na.out -> b.in\n}\n");

    let units = fmt(&dir.0.join("root.fpp")).expect("format should succeed");

    // Root + three fragments, each formatted exactly once.
    assert_eq!(units.len(), 4, "unexpected units: {:?}", units);

    let by_name = |name: &str| {
        units
            .iter()
            .find(|u| u.path.file_name().unwrap() == name)
            .unwrap_or_else(|| panic!("missing unit {name}"))
    };

    // Entrypoints derived from context.
    assert_eq!(by_name("root.fpp").entry, TopEntryPoint::Module);
    assert_eq!(by_name("consts.fppi").entry, TopEntryPoint::Module);
    assert_eq!(by_name("ports.fppi").entry, TopEntryPoint::Component);
    assert_eq!(by_name("conns.fppi").entry, TopEntryPoint::Topology);

    // The module fragment's constants get aligned on `=`.
    assert_eq!(
        by_name("consts.fppi").formatted,
        "constant a    = 1\nconstant bbbb = 2\n"
    );

    // Every formatted fragment must reparse cleanly under its derived entry.
    for u in &units {
        let p = fpp_lsp_parser::parse(&u.formatted, u.entry);
        assert!(
            p.errors().is_empty(),
            "{}: reparse errors {:?}\n{}",
            u.path.display(),
            p.errors(),
            u.formatted
        );
    }
}

#[test]
fn resolves_relative_to_including_file() {
    let dir = TmpDir::new();
    // root includes sub/a.fppi, which itself includes b.fppi resolved relative
    // to sub/ (not the root dir).
    dir.write("root.fpp", "module M {\ninclude \"sub/a.fppi\"\n}\n");
    dir.write("sub/a.fppi", "include \"b.fppi\"\nconstant a = 1\n");
    dir.write("sub/b.fppi", "constant b = 2\n");

    let units = fmt(&dir.0.join("root.fpp")).expect("format should succeed");
    assert_eq!(units.len(), 3, "unexpected units: {:?}", units);
    assert!(units.iter().any(|u| u.path.ends_with("sub/b.fppi")));
}

#[test]
fn formats_shared_fragment_once() {
    let dir = TmpDir::new();
    dir.write(
        "root.fpp",
        "module M {\ninclude \"a.fppi\"\ninclude \"b.fppi\"\n}\n",
    );
    dir.write("a.fppi", "include \"shared.fppi\"\nconstant a = 1\n");
    dir.write("b.fppi", "include \"shared.fppi\"\nconstant b = 2\n");
    dir.write("shared.fppi", "constant s = 3\n");

    let units = fmt(&dir.0.join("root.fpp")).expect("format should succeed");
    let shared_count = units
        .iter()
        .filter(|u| u.path.file_name().unwrap() == "shared.fppi")
        .count();
    assert_eq!(shared_count, 1, "shared fragment formatted more than once");
}

#[test]
fn detects_include_cycle() {
    let dir = TmpDir::new();
    dir.write("root.fpp", "module M {\ninclude \"a.fppi\"\n}\n");
    dir.write("a.fppi", "include \"b.fppi\"\n");
    dir.write("b.fppi", "include \"a.fppi\"\n");

    match fmt(&dir.0.join("root.fpp")) {
        Err(FormatError::IncludeCycle(chain)) => {
            assert!(chain.len() >= 2, "cycle chain too short: {:?}", chain);
        }
        other => panic!("expected IncludeCycle, got {:?}", other),
    }
}

#[test]
fn formats_state_machine_and_state_includes() {
    let dir = TmpDir::new();
    // An include in a state machine is a fragment of state machine members,
    // and an include in a state is a fragment of state members.
    dir.write(
        "root.fpp",
        "module M {\nstate machine S {\ninclude \"sm.fppi\"\nstate B {\ninclude \"state.fppi\"\n}\n}\n}\n",
    );
    dir.write("sm.fppi", "initial enter A\nstate A {   entry do { a } }\n");
    dir.write("state.fppi", "entry   do { a }\non s enter A\n");

    let units = fmt(&dir.0.join("root.fpp")).expect("format should succeed");
    assert_eq!(units.len(), 3, "unexpected units: {:?}", units);
    let by_name = |name: &str| {
        units
            .iter()
            .find(|u| u.path.file_name().unwrap() == name)
            .unwrap_or_else(|| panic!("missing unit {name}"))
    };
    assert_eq!(by_name("sm.fppi").entry, TopEntryPoint::StateMachine);
    assert_eq!(by_name("state.fppi").entry, TopEntryPoint::State);
    assert_eq!(
        by_name("sm.fppi").formatted,
        "initial enter A\nstate A {\n    entry do { a }\n}\n"
    );
    assert_eq!(
        by_name("state.fppi").formatted,
        "entry do { a }\non s enter A\n"
    );
}

#[test]
fn nested_include_inherits_fragment_entry() {
    let dir = TmpDir::new();
    // An include at the top of a component fragment is itself a component
    // fragment.
    dir.write(
        "top.fpp",
        "module M {\n  passive component C {\n    include \"a.fppi\"\n  }\n}\n",
    );
    dir.write("a.fppi", "include \"b.fppi\"\n");
    dir.write("b.fppi", "event E severity activity high format \"x\"\n");

    let units = fmt(&dir.0.join("top.fpp")).expect("format should succeed");
    assert_eq!(units.len(), 3, "unexpected units: {:?}", units);
    for u in &units[1..] {
        assert_eq!(u.entry, TopEntryPoint::Component, "{}", u.path.display());
    }
}

#[test]
fn formats_packet_and_packet_set_includes() {
    let dir = TmpDir::new();
    dir.write(
        "top.fpp",
        "module M { topology T { telemetry packets P {\n include \"set.fppi\"\n } } }\n",
    );
    dir.write(
        "set.fppi",
        "@ Included packet\npacket P2 id 1 group 1 {\n  i4.c4\n  include \"channel.fppi\"\n}\n@< Included packet\n",
    );
    dir.write("channel.fppi", "i5.c5\n");

    let units = fmt(&dir.0.join("top.fpp")).expect("format should succeed");
    assert_eq!(units.len(), 3, "unexpected units: {:?}", units);
    let set = units
        .iter()
        .find(|u| u.path.file_name().unwrap() == "set.fppi")
        .unwrap();
    assert_eq!(set.entry, TopEntryPoint::TlmPacketSet);
    assert_eq!(
        set.formatted,
        "@ Included packet\npacket P2 id 1 group 1 {\n    i4.c4\n    include \"channel.fppi\"\n}  @< Included packet\n"
    );
    let channel = units
        .iter()
        .find(|u| u.path.file_name().unwrap() == "channel.fppi")
        .unwrap();
    assert_eq!(channel.entry, TopEntryPoint::TlmPacket);
    assert_eq!(channel.formatted, "i5.c5\n");
}

#[test]
fn missing_include_is_io_error() {
    let dir = TmpDir::new();
    dir.write("root.fpp", "module M {\ninclude \"nope.fppi\"\n}\n");
    match fmt(&dir.0.join("root.fpp")) {
        Err(FormatError::IoError(_)) => {}
        other => panic!("expected IoError, got {:?}", other),
    }
}

#[test]
fn writes_are_not_performed_by_formatter() {
    // format_file_recursive must not mutate files; it only returns results.
    let dir = TmpDir::new();
    dir.write("root.fpp", "module M {\ninclude \"c.fppi\"\n}\n");
    let orig = "constant a = 1\nconstant bbbb = 2\n";
    dir.write("c.fppi", orig);

    let _ = fmt(&dir.0.join("root.fpp")).expect("format should succeed");
    assert_eq!(dir.read("c.fppi"), orig, "formatter must not write files");
}
