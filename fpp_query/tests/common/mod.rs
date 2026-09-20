//! Shared harness for the end-to-end suites, which all drive the real `fpp-query`
//! binary. Cargo exposes its path as `CARGO_BIN_EXE_<name>`.
//!
//! There is one way to give the tool rules — a `--rules` file — so every suite
//! writes one, and [`query`] is the shape almost every test wants.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_fpp-query")
}

/// A throwaway directory under the crate's target dir, removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    /// The pid is part of the name because cargo runs the suites as separate
    /// processes in parallel, and `new` starts by removing the directory: two
    /// suites picking the same tag would delete each other's files.
    pub fn new(tag: &str) -> TempDir {
        let mut path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
        path.push(format!("fpp_query_{}_{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Write `content` to `name` inside the directory and return its path.
    pub fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    pub fn str(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn run(args: &[&str]) -> Output {
    Command::new(bin()).args(args).output().unwrap()
}

/// Write `rules` and `model` into `dir`, then select from one against the other.
pub fn query(dir: &TempDir, rules: &str, model: &str) -> Output {
    let model = dir.write("m.fpp", model);
    let rules = dir.write("rules.toml", rules);
    run(&[
        "--rules",
        &rules.to_string_lossy(),
        "-d",
        &dir.str(),
        "--",
        &model.to_string_lossy(),
    ])
}

/// One group, as the `[[group]]` body. The common case: `node`, maybe a `where` or
/// `name`, and `generate`.
pub fn group(dir: &TempDir, body: &str, model: &str) -> Output {
    query(dir, &format!("[[group]]\n{body}\n"), model)
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Stdout of a successful run, as lines.
pub fn lines(output: &Output) -> Vec<String> {
    assert!(
        output.status.success(),
        "expected success, got {:?}\nstderr:\n{}",
        output.status,
        stderr(output)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

/// Emitted filenames with the output directory stripped, so assertions read as the
/// bare names the autocoder would produce.
pub fn names(output: &Output, dir: &Path) -> Vec<String> {
    let prefix = format!("{}/", dir.display());
    lines(output)
        .into_iter()
        .map(|l| l.strip_prefix(&prefix).unwrap_or(&l).to_string())
        .collect()
}

/// A model exercising every nesting level a definition can appear at.
pub const MODEL: &str = "\
module M {
  @ static-tlm-packetizer
  passive component C {
    array A = [3] U32
    enum E { X, Y }
    struct S { x: U32 }
    state machine SM {
      array A = [2] U8
      initial enter S1
      state S1
    }
  }
  active component Act { }
  array A = [4] U8
  type Abs
  type Alias = U32
  deployment topology T1 { }
  topology T2 { }
}
";
