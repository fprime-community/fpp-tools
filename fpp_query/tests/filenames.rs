//! `fpp-filenames`'s test suite.
//!
//! The models in `tests/filenames/*.fpp` and the reference outputs in
//! `tests/filenames/*.ref.txt` are copied verbatim from the Scala compiler's
//! `compiler/tools/fpp-filenames/test`, and the cases below mirror that suite's
//! `run` script one for one.
//!
//! `FPP_UPDATE_REF=1 cargo test -p fpp_query --test filenames` rewrites the
//! references.

use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_fpp-query")
}

fn dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("filenames");
    path
}

/// The `fpp-filenames` modes, as the presets that reproduce each one. Upstream
/// spells these as flags; here each is a rules file.
#[derive(Clone, Copy)]
enum Mode {
    /// No flags: the autocode files.
    Autocode,
    /// The autocode files again, from the *expanded* model. Shares every reference
    /// with [`Mode::Autocode`], which is the point: hand-encoding the synthesized
    /// `State` enum and expanding the model to contain it must agree exactly, or one
    /// of the two spellings is wrong about what `fpp-to-cpp` writes.
    AutocodeExpanded,
    /// `-t`: the implementation template for a component.
    Template,
    /// `-u`: the unit-test base classes.
    Test,
    /// `-u -a`: ...plus the auto-generated test helpers.
    TestAutoHelpers,
    /// `-u -t`: the unit-test template. Helpers are included here, because
    /// upstream emits them when `-a` is *off*.
    TestTemplate,
    /// `-u -t -a`: the unit-test template without the helpers.
    TestTemplateAutoHelpers,
}

impl Mode {
    fn preset(self) -> PathBuf {
        let name = match self {
            Mode::Autocode => "autocode",
            Mode::AutocodeExpanded => "autocode-expanded",
            Mode::Template => "template",
            Mode::Test => "test",
            Mode::TestAutoHelpers => "test-auto-helpers",
            Mode::TestTemplate => "test-template",
            Mode::TestTemplateAutoHelpers => "test-template-auto-helpers",
        };
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("presets");
        path.push(format!("{name}.toml"));
        path
    }
}

/// Run `<input>.fpp` in `mode` and return the bare generated filenames, one per
/// line — upstream's output format.
///
/// Stripping the `-d` directory is this suite's `remove_path_prefix`: upstream
/// writes relative names and resolves them against its output directory, while this
/// tool writes the absolute paths CMake needs.
fn generate(mode: Mode, input: &str) -> String {
    let dir = dir();
    let out = Command::new(bin())
        .arg("-d")
        .arg(&dir)
        .arg("--rules")
        .arg(mode.preset())
        .arg("--")
        .arg(dir.join(format!("{input}.fpp")))
        .output()
        .expect("failed to run fpp-query");
    assert!(
        out.status.success(),
        "{input}.fpp failed: {:?}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let prefix = format!("{}/", dir.display());
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|line| {
            line.strip_prefix(&prefix)
                .unwrap_or_else(|| panic!("{line} is not under the output directory"))
        })
        .fold(String::new(), |mut acc, name| {
            acc.push_str(name);
            acc.push('\n');
            acc
        })
}

/// Compare `<input>.fpp` in `mode` against the reference the case owns.
fn run_test(mode: Mode, input: &str, reference: &str) {
    let output = generate(mode, input);
    let ref_file = dir().join(format!("{reference}.ref.txt"));
    if env::var("FPP_UPDATE_REF").is_ok() {
        fs::write(&ref_file, output).expect("failed to write ref.txt");
        return;
    }
    let expected = fs::read_to_string(&ref_file)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", ref_file.display()));
    assert_eq!(expected, output);
}

// `include.fpp` is `include "ok.fpp"`, so every mode must produce exactly what
// `ok.fpp` does: a textual splice contributes filenames like anything else.

#[test]
fn include() {
    run_test(Mode::Autocode, "include", "include");
}

#[test]
fn include_template() {
    run_test(Mode::Template, "include", "include_template");
}

#[test]
fn include_test() {
    run_test(Mode::Test, "include", "include_test");
}

#[test]
fn include_test_auto_helpers() {
    run_test(
        Mode::TestAutoHelpers,
        "include",
        "include_test_auto_helpers",
    );
}

#[test]
fn include_test_template() {
    run_test(Mode::TestTemplate, "include", "include_test_template");
}

#[test]
fn include_test_template_auto_helpers() {
    run_test(
        Mode::TestTemplateAutoHelpers,
        "include",
        "include_test_template_auto_helpers",
    );
}

#[test]
fn ok() {
    run_test(Mode::Autocode, "ok", "ok");
}

// The expanded model reaches the same reference. `ok.fpp` covers all three shapes
// that matters for: an external `state machine SM1` (no enum), a module-level
// `state machine SM2` (`SM2_StateEnumAc.*`), and one nested in a component
// (`C_SM_StateEnumAc.*`).

#[test]
fn ok_expanded() {
    run_test(Mode::AutocodeExpanded, "ok", "ok");
}

#[test]
fn include_expanded() {
    run_test(Mode::AutocodeExpanded, "include", "include");
}

#[test]
fn ok_template() {
    run_test(Mode::Template, "ok", "ok_template");
}

#[test]
fn ok_test() {
    run_test(Mode::Test, "ok", "ok_test");
}

#[test]
fn ok_test_auto_helpers() {
    run_test(Mode::TestAutoHelpers, "ok", "ok_test_auto_helpers");
}

#[test]
fn ok_test_template() {
    run_test(Mode::TestTemplate, "ok", "ok_test_template");
}

#[test]
fn ok_test_template_auto_helpers() {
    run_test(
        Mode::TestTemplateAutoHelpers,
        "ok",
        "ok_test_template_auto_helpers",
    );
}
