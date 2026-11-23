//! The analysis pipeline the `fpp-diagram` binary runs, which is a property of
//! the process rather than of a function, so it is tested by running the binary.
//! Cargo exposes its path as `CARGO_BIN_EXE_<name>`.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

/// Run `fpp-diagram` with `args`, feeding `model` on stdin.
fn run(args: &[&str], model: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fpp-diagram"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(model.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

/// Everything the process wrote, on either stream.
fn output_text(out: &Output) -> String {
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    text
}

/// A unique temp dir per test without pulling in a tempfile dependency.
fn temp_dir(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("fpp-diagram-cli-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An F´ project whose topology uses a component defined in another file, which
/// only the locs file of its native build cache locates.
fn project(tag: &str) -> PathBuf {
    let dir = temp_dir(tag);
    std::fs::create_dir_all(dir.join("Comp")).unwrap();
    std::fs::create_dir_all(dir.join("Top")).unwrap();
    std::fs::create_dir_all(dir.join("build-fprime-automatic-native")).unwrap();
    std::fs::write(dir.join("settings.ini"), "[fprime]\nproject_root: .\n").unwrap();
    std::fs::write(
        dir.join("Comp/C.fpp"),
        "passive component C { }\ninstance c: C base id 0x100\n",
    )
    .unwrap();
    std::fs::write(dir.join("Top/T.fpp"), "topology T { instance c }\n").unwrap();
    std::fs::write(
        dir.join("build-fprime-automatic-native/locs.fpp"),
        "locate component C at \"../Comp/C.fpp\"\n\
         locate instance c at \"../Comp/C.fpp\"\n\
         locate interface T at \"../Top/T.fpp\"\n",
    )
    .unwrap();
    dir
}

const TOPOLOGY: &[&str] = &["--name", "T", "--kind", "topology", "--format", "sprotty"];

#[test]
fn the_locs_file_of_the_enclosing_project_is_detected() {
    let dir = project("detect");
    let file = dir.join("Top/T.fpp");
    let out = run(&[TOPOLOGY, &[file.to_str().unwrap()]].concat(), "");
    let text = output_text(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("\"text\":\"C\""), "{text}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_explicit_locs_file_is_used() {
    let dir = project("explicit");
    std::fs::remove_file(dir.join("settings.ini")).unwrap();
    let locs = dir.join("build-fprime-automatic-native/locs.fpp");
    let file = dir.join("Top/T.fpp");
    let out = run(
        &[
            TOPOLOGY,
            &["--locs", locs.to_str().unwrap(), file.to_str().unwrap()],
        ]
        .concat(),
        "",
    );
    let text = output_text(&out);
    assert_eq!(out.status.code(), Some(0), "{text}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn no_locs_analyzes_the_input_alone() {
    let dir = project("no-locs");
    let file = dir.join("Top/T.fpp");
    let out = run(
        &[TOPOLOGY, &["--no-locs", file.to_str().unwrap()]].concat(),
        "",
    );
    let text = output_text(&out);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("is not defined"), "{text}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Template resolution failed, so the model is not expanded and nothing in it can
/// be checked: the semantic check must not run and report the names the missing
/// expansion would have defined.
#[test]
fn a_failed_template_resolution_stops_the_analysis() {
    let out = run(
        &["--name", "Z", "--kind", "topology"],
        "module template T(constant p: U32, constant q: U32) { constant c = p + q }\n\
         module M { expand T(constant 1) }\n\
         array Z = [M.c] U32\n",
    );
    let text = output_text(&out);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(
        text.contains("expected 2 template arguments, got 1"),
        "{text}"
    );
    assert!(
        !text.contains("is not defined"),
        "the semantic check ran anyway: {text}"
    );
}
