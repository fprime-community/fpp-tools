//! The analysis pipeline the `fpp-diagram` binary runs, which is a property of
//! the process rather than of a function, so it is tested by running the binary.
//! Cargo exposes its path as `CARGO_BIN_EXE_<name>`.

use std::io::Write;
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
