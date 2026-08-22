//! End-to-end tests for `.fpp-format` discovery and precedence, driving the
//! actual `fpp-format` binary. Cargo exposes its path as `CARGO_BIN_EXE_<name>`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Path to the compiled `fpp-format` binary under test.
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_fpp-format")
}

/// Leading-whitespace width of the first indented (`constant`) line.
fn indent_of(text: &str) -> usize {
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with("constant"))
        .unwrap_or_else(|| panic!("no constant line in:\n{}", text));
    line.len() - line.trim_start().len()
}

/// A throwaway directory under the crate's target dir, removed on drop. Avoids
/// a `tempfile` dependency while keeping each test hermetic.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        let mut path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
        path.push(format!("fpp_format_cfg_{tag}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const UNFORMATTED: &str = "module M {\nconstant x = 1\n}\n";

#[test]
fn default_indent_is_four_without_config() {
    let dir = TempDir::new("default");
    let file = dir.path().join("a.fpp");
    fs::write(&file, UNFORMATTED).unwrap();

    let status = Command::new(bin()).arg(&file).status().unwrap();
    assert!(status.success());
    assert_eq!(indent_of(&fs::read_to_string(&file).unwrap()), 4);
}

#[test]
fn config_file_is_discovered_by_walking_up() {
    let dir = TempDir::new("walkup");
    // Config at the root, file nested two levels down.
    fs::write(dir.path().join(".fpp-format"), "indent = 4\n").unwrap();
    let nested = dir.path().join("sub/deeper");
    fs::create_dir_all(&nested).unwrap();
    let file = nested.join("a.fpp");
    fs::write(&file, UNFORMATTED).unwrap();

    let status = Command::new(bin()).arg(&file).status().unwrap();
    assert!(status.success());
    assert_eq!(indent_of(&fs::read_to_string(&file).unwrap()), 4);
}

#[test]
fn cli_flag_overrides_config_file() {
    let dir = TempDir::new("override");
    fs::write(dir.path().join(".fpp-format"), "indent = 4\n").unwrap();
    let file = dir.path().join("a.fpp");
    fs::write(&file, UNFORMATTED).unwrap();

    let status = Command::new(bin())
        .arg("--indent")
        .arg("8")
        .arg(&file)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(indent_of(&fs::read_to_string(&file).unwrap()), 8);
}

#[test]
fn malformed_config_is_a_hard_error() {
    let dir = TempDir::new("malformed");
    fs::write(dir.path().join(".fpp-format"), "bogus = 1\n").unwrap();
    let file = dir.path().join("a.fpp");
    fs::write(&file, UNFORMATTED).unwrap();

    let output = Command::new(bin()).arg(&file).output().unwrap();
    assert!(!output.status.success(), "expected non-zero exit");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown key"), "stderr:\n{}", stderr);
    // The file must be left untouched when the config is rejected.
    assert_eq!(fs::read_to_string(&file).unwrap(), UNFORMATTED);
}
