//! The process contract CMake sees, and the two reporting modes.
//!
//! Exit codes, the `--filenames` file's existence and format, `-d` resolution,
//! `--json`, and `--fields` are properties of the process rather than of a function,
//! so they are tested by running the binary. What a query *means* lives in
//! `query.rs`; the rules file's own surface lives in `rules.rs`.

mod common;

use common::{MODEL, TempDir, bin, group, lines, names, run, stderr};
use std::fs;
use std::path::Path;
use std::process::Command;

/// The rules file every contract test here uses. It is deliberately a single-match
/// rule set, so a contract about the *output* is not confounded by how many
/// definitions matched.
const RULES: &str = "[[group]]\n\
    node = \"DefTopology\"\n\
    where = '$.is_deployment'\n\
    generate = [\"TopologyAc.hpp\"]\n";

/// Run with `--filenames`, returning the output and the list file's path.
fn with_filenames(
    dir: &TempDir,
    rules: &str,
    model: &str,
) -> (std::process::Output, std::path::PathBuf) {
    let model = dir.write("m.fpp", model);
    let rules = dir.write("rules.toml", rules);
    let list = dir.path().join("names.txt");
    let out = run(&[
        "--rules",
        &rules.to_string_lossy(),
        "-d",
        &dir.str(),
        "--filenames",
        &list.to_string_lossy(),
        "--",
        &model.to_string_lossy(),
    ]);
    (out, list)
}

#[test]
fn no_rules_at_all_is_rejected() {
    let out = run(&["--", "x.fpp"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("no rules given"), "{}", stderr(&out));
}

/// `file(STRINGS)` on a missing file is a hard CMake FATAL_ERROR, and no matches is
/// not an error, so this must be exit 0 with an empty file.
#[test]
fn output_file_is_created_even_with_no_matches() {
    let dir = TempDir::new("empty");
    let (out, list) = with_filenames(&dir, RULES, "module M {\n  constant x = 1\n}\n");
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(fs::read_to_string(&list).unwrap(), "");
    assert!(out.stdout.is_empty(), "--filenames must not also print");
}

#[test]
fn output_file_is_lf_terminated_with_no_blank_lines() {
    let dir = TempDir::new("format");
    let (_, list) = with_filenames(
        &dir,
        "[[group]]\nnode = \"DefArray\"\ngenerate = [\"ArrayAc.hpp\", \"ArrayAc.cpp\"]\n",
        MODEL,
    );
    let text = fs::read_to_string(&list).unwrap();
    assert!(text.ends_with('\n'), "must be LF-terminated");
    // A blank line would become an empty CMake list element, hence an empty
    // `add_custom_command(OUTPUT ...)` entry.
    assert!(!text.contains("\n\n"), "must not contain a blank line");
    for line in text.lines() {
        assert!(!line.is_empty());
        assert!(
            Path::new(line).is_absolute(),
            "paths go straight into add_custom_command(OUTPUT ...): {line}"
        );
    }
}

#[test]
fn syntax_error_exits_one_writes_the_list() {
    let dir = TempDir::new("syntaxerr");
    let (out, list) = with_filenames(&dir, RULES, "module M { this is not fpp }\n");
    assert_eq!(out.status.code(), Some(1));
    // Diagnostics on stderr, because CMake captures stdout separately and the file
    // list lives there.
    assert!(!out.stderr.is_empty());
    // The file exists, so CMake reports our diagnostic rather than its own read
    // failure.
    assert!(list.exists());
}

#[test]
fn unreadable_input_exits_two() {
    let dir = TempDir::new("unreadable");
    let rules = dir.write("rules.toml", RULES);
    let out = run(&[
        "--rules",
        &rules.to_string_lossy(),
        "--",
        "/nonexistent/nope.fpp",
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("cannot read"), "{}", stderr(&out));
}

/// The build-time invocation passes `-i`; the configure-time one is given the same
/// argv and must ignore it, since an import cannot change a syntax-only answer.
#[test]
fn imports_accepted_and_ignored() {
    let dir = TempDir::new("imports");
    let model = dir.write("m.fpp", MODEL);
    let rules = dir.write("rules.toml", RULES);

    let without = run(&[
        "--rules",
        &rules.to_string_lossy(),
        "-d",
        &dir.str(),
        "--",
        &model.to_string_lossy(),
    ]);
    let with = run(&[
        "--rules",
        &rules.to_string_lossy(),
        "-d",
        &dir.str(),
        "-i",
        "/nonexistent/a.fpp,/nonexistent/b.fpp",
        "--",
        &model.to_string_lossy(),
    ]);
    assert_eq!(lines(&without), lines(&with));
    assert_eq!(names(&without, dir.path()), ["T1TopologyAc.hpp"]);
}

#[test]
fn duplicate_input_paths_do_not_duplicate_output() {
    let dir = TempDir::new("dupinput");
    let model = dir.write("m.fpp", MODEL);
    let rules = dir.write("rules.toml", RULES);
    let args = |files: &[&str]| {
        let mut argv = vec![
            "--rules".to_string(),
            rules.to_string_lossy().into_owned(),
            "-d".to_string(),
            dir.str(),
            "--".to_string(),
        ];
        argv.extend(files.iter().map(|f| (*f).to_string()));
        let borrowed: Vec<&str> = argv.iter().map(String::as_str).collect();
        run(&borrowed)
    };
    let path = model.to_string_lossy().into_owned();
    assert_eq!(lines(&args(&[&path])), lines(&args(&[&path, &path])));
}

/// `.` and `..` are collapsed without touching the filesystem, matching CMake's
/// `get_filename_component(... ABSOLUTE)`.
#[test]
fn relative_output_directory_is_absolutized() {
    let dir = TempDir::new("relative");
    let model = dir.write("m.fpp", MODEL);
    let rules = dir.write("rules.toml", RULES);
    let out = Command::new(bin())
        .current_dir(dir.path())
        .args([
            "--rules".as_ref(),
            rules.as_os_str(),
            "-d".as_ref(),
            "./build/./sub/..".as_ref(),
            "--".as_ref(),
            model.as_os_str(),
        ])
        .output()
        .unwrap();
    let emitted = lines(&out);
    assert_eq!(emitted.len(), 1);
    assert!(Path::new(&emitted[0]).is_absolute(), "{}", emitted[0]);
    assert!(
        emitted[0].ends_with("/build/T1TopologyAc.hpp"),
        "{}",
        emitted[0]
    );
}

/// A node handle and a span index only mean something inside the compiler context
/// that produced them, so neither is part of the queryable surface.
#[test]
fn json_dump_omits_context_handles() {
    let dir = TempDir::new("json");
    let model = dir.write("m.fpp", MODEL);
    let out = run(&["--json", &model.to_string_lossy()]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("\"is_deployment\""));
    assert!(!text.contains("node_id"), "node_id must not be serialized");
    assert!(!text.contains("inner_span"), "spans must not be serialized");
}

/// `MODEL` puts `@ static-tlm-packetizer` on `component C`; `--json` must show it,
/// since it claims to dump "exactly what a query sees" and `$@pre` sees it.
#[test]
fn json_dump_includes_annotations() {
    let dir = TempDir::new("json_annot");
    let model = dir.write("m.fpp", MODEL);
    let out = run(&["--json", &model.to_string_lossy()]);
    assert!(out.status.success());
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();

    fn find<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
        match value {
            serde_json::Value::Object(map) => map
                .get(key)
                .or_else(|| map.values().find_map(|v| find(v, key))),
            serde_json::Value::Array(items) => items.iter().find_map(|v| find(v, key)),
            _ => None,
        }
    }

    let component = find(&value, "DefComponent").expect("a DefComponent");
    let pre = component
        .get("annotations")
        .and_then(|a| a.get("pre"))
        .expect("annotations.pre");
    assert_eq!(
        pre,
        &serde_json::json!(["static-tlm-packetizer"]),
        "full dump: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn fields_lists_kinds_without_any_input() {
    let out = run(&["--fields"]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("DefTopology"));
    assert!(text.contains("DefArray"));
    // The transparent wrapper is not offered, since it can never match.
    assert!(!text.contains("SpecPortInstance\n"));
}

/// The reporting modes answer about the model, so they work with no rules at all —
/// the one exception to `no_rules_at_all_is_rejected`.
#[test]
fn reporting_modes_need_no_rules() {
    let dir = TempDir::new("reporting");
    let model = dir.write("m.fpp", MODEL);
    for args in [
        vec!["--json", &model.to_string_lossy()],
        vec!["--fields", "DefTopology", &model.to_string_lossy()],
    ] {
        let out = run(&args.iter().map(AsRef::as_ref).collect::<Vec<&str>>());
        assert!(out.status.success(), "{:?}: {}", args, stderr(&out));
        assert!(!out.stdout.is_empty());
    }
}

/// Sanity check that [`RULES`] selects the one definition the contract tests above
/// assume, so a failure there is about the process and not about selection.
#[test]
fn shared_rules_select_one_definition() {
    let dir = TempDir::new("sanity");
    let out = group(
        &dir,
        "node = \"DefTopology\"\nwhere = '$.is_deployment'\ngenerate = [\"TopologyAc.hpp\"]",
        MODEL,
    );
    assert_eq!(names(&out, dir.path()), ["T1TopologyAc.hpp"]);
}
