//! The `--rules` TOML surface, driving the actual binary.

mod common;

use common::{TempDir, bin, names, query, run, stderr};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// A smaller model than the shared one: these cases care about the rules file, so
/// the assertions read better against a model with one definition of each kind.
const MODEL: &str = "\
module M {
  passive component C {
    struct S { x: U32 }
    state machine SM {
      initial enter S1
      state S1
    }
  }
  type Alias = U32
  deployment topology T1 { }
  topology T2 { }
}
";

/// Run `rules` against [`MODEL`].
fn rules(dir: &TempDir, rules: &str) -> std::process::Output {
    query(dir, rules, MODEL)
}

#[test]
fn groups_are_independent_and_the_output_is_sorted() {
    let dir = TempDir::new("groups");
    let out = rules(
        &dir,
        r#"
[[group]]
node = "DefStruct"
generate = ["SerializableAc.hpp"]

[[group]]
node = "DefAliasType"
generate = ["AliasAc.hpp"]

[[group]]
node = "DefTopology"
where = '$.is_deployment'
generate = ["TopologyAc.hpp"]
"#,
    );
    assert_eq!(
        names(&out, dir.path()),
        [
            "AliasAliasAc.hpp",
            "C_SSerializableAc.hpp",
            "T1TopologyAc.hpp"
        ]
    );
}

#[test]
fn a_name_expression_reaches_the_enclosing_definition() {
    let dir = TempDir::new("name");
    let out = rules(
        &dir,
        r#"
[[group]]
node = "DefStateMachine"
where = '$.members != null'
name = '$@stem + "_State"'
generate = ["EnumAc.hpp"]
"#,
    );
    assert_eq!(names(&out, dir.path()), ["C_SM_StateEnumAc.hpp"]);
}

/// The reason a query lives in the rules file rather than being re-parsed in
/// isolation: the caret lands on the real line and column of the sub-expression.
#[test]
fn a_query_error_points_into_the_rules_file() {
    let dir = TempDir::new("caret");
    let out = rules(
        &dir,
        r#"[[group]]
node = "DefTopology"
where = '$.is_deploy'
generate = ["Ac.hpp"]
"#,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    assert!(text.contains("has no field `is_deploy`"), "{text}");
    // Line 3 of rules.toml, not a synthetic `<--where #1>`.
    assert!(text.contains("rules.toml:3:"), "{text}");
    // The caret covers `is_deploy` alone, so it is 9 columns wide.
    assert!(text.contains("^^^^^^^^^"), "{text}");
}

#[test]
fn a_multi_line_query_keeps_its_columns() {
    let dir = TempDir::new("multiline");
    let out = rules(
        &dir,
        r#"[[group]]
node = "DefTopology"
where = '''
  $.is_deployment
  && $.nope'''
generate = ["Ac.hpp"]
"#,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    // The offending field is on the fifth line of the file, not the first line of
    // the query.
    assert!(text.contains("rules.toml:5:"), "{text}");
    assert!(text.contains("has no field `nope`"), "{text}");
}

/// A basic string's escapes decode to something shorter than the source, so every
/// caret past the first escape would point at the wrong column. Rejected rather
/// than mis-carated.
#[test]
fn an_escaped_query_string_is_rejected_with_the_literal_spelling() {
    let dir = TempDir::new("escaped");
    let out = rules(
        &dir,
        r#"[[group]]
node = "DefStateMachine"
name = "$@stem + \"_State\""
generate = ["EnumAc.hpp"]
"#,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    assert!(text.contains("`name` uses TOML escape sequences"), "{text}");
    assert!(text.contains("literal string"), "{text}");
}

/// An unescaped basic string is fine: nothing decoded, so the offsets hold.
#[test]
fn an_unescaped_basic_string_is_accepted() {
    let dir = TempDir::new("basic");
    let out = rules(
        &dir,
        r#"
[[group]]
node = "DefTopology"
where = "$.is_deployment"
generate = ["Ac.hpp"]
"#,
    );
    assert_eq!(names(&out, dir.path()), ["T1Ac.hpp"]);
}

#[test]
fn a_misspelled_key_names_the_real_ones() {
    let dir = TempDir::new("badkey");
    let out = rules(
        &dir,
        r#"[[group]]
node = "DefTopology"
wheer = '$.is_deployment'
generate = ["Ac.hpp"]
"#,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    assert!(text.contains("unknown field `wheer`"), "{text}");
    // The alternatives, exactly, rather than a guess at which was meant.
    assert!(text.contains("`where`"), "{text}");
    assert!(text.contains("rules.toml:3:"), "{text}");
}

#[test]
fn a_missing_key_is_reported_against_the_group() {
    let dir = TempDir::new("missingkey");
    let out = rules(
        &dir,
        r#"[[group]]
node = "DefTopology"
"#,
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("missing field `generate`"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn an_unknown_kind_is_carated_in_the_file() {
    let dir = TempDir::new("badkind");
    let out = rules(
        &dir,
        r#"[[group]]
node = "DefTopolgy"
generate = ["Ac.hpp"]
"#,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    assert!(text.contains("is not an AST node kind"), "{text}");
    assert!(text.contains("rules.toml:2:"), "{text}");
    // The caret covers the kind name, not the surrounding quotes.
    assert!(text.contains("^^^^^^^^^^"), "{text}");
}

#[test]
fn a_transparent_kind_is_rejected_rather_than_matching_nothing() {
    let dir = TempDir::new("transparent");
    let out = rules(
        &dir,
        r#"[[group]]
node = "SpecPortInstance"
generate = ["Ac.hpp"]
"#,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    assert!(text.contains("transparent wrapper"), "{text}");
    assert!(text.contains("SpecGeneralPortInstance"), "{text}");
}

#[test]
fn an_empty_generate_list_is_rejected() {
    let dir = TempDir::new("emptygen");
    let out = rules(
        &dir,
        r#"[[group]]
node = "DefTopology"
generate = []
"#,
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("generates nothing"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_suffix_that_would_corrupt_the_list_is_rejected() {
    let dir = TempDir::new("badsuffix");
    for suffix in ["a/b.hpp", "a;b.hpp", ""] {
        let out = rules(
            &dir,
            &format!("[[group]]\nnode = \"DefTopology\"\ngenerate = [\"{suffix}\"]\n"),
        );
        assert_eq!(out.status.code(), Some(1), "`{suffix}` was accepted");
        let text = stderr(&out);
        assert!(
            text.contains("cannot appear in a generated filename")
                || text.contains("cannot be empty"),
            "`{suffix}`: {text}"
        );
    }
}

/// One run reports every rule the file itself gets wrong, so several mistakes take
/// one round trip to fix rather than one each.
///
/// This covers what reading the file can decide: the kind, the suffixes, and
/// whether a query parses. Whether a query *evaluates* needs a node to evaluate it
/// against, so an unknown field surfaces later, during the walk.
#[test]
fn every_bad_group_is_reported_in_one_run() {
    let dir = TempDir::new("allerrors");
    let out = rules(
        &dir,
        r#"[[group]]
node = "DefNope"
generate = ["Ac.hpp"]

[[group]]
node = "DefTopology"
where = '$.is_deployment =='
generate = ["Ac.hpp"]

[[group]]
node = "DefArray"
generate = ["a/b.hpp"]
"#,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    assert!(text.contains("is not an AST node kind"), "{text}");
    assert!(text.contains("end of query"), "{text}");
    assert!(
        text.contains("cannot appear in a generated filename"),
        "{text}"
    );
}

#[test]
fn a_rules_file_with_no_groups_is_rejected() {
    let dir = TempDir::new("nogroups");
    let out = rules(&dir, "# nothing here yet\n");
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("defines no `[[group]]`"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn malformed_toml_is_reported_with_a_caret() {
    let dir = TempDir::new("badtoml");
    let out = rules(&dir, "[[group]\nnode = \"DefTopology\"\n");
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("rules.toml:1:"), "{}", stderr(&out));
}

#[test]
fn an_unreadable_rules_file_exits_two() {
    let out = Command::new(bin())
        .args(["--rules", "/nonexistent/rules.toml", "--", "x.fpp"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("cannot read"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A rules-file error is diagnosed before any model is parsed, and the
/// `--filenames` file must still exist afterwards: `file(STRINGS)` on a missing
/// file is a hard CMake `FATAL_ERROR`, which would turn one reportable error into
/// two.
#[test]
fn the_filenames_file_survives_a_rules_error() {
    let dir = TempDir::new("contract");
    let model = dir.write("m.fpp", MODEL);
    let rules = dir.write(
        "rules.toml",
        "[[group]]\nnode = \"DefNope\"\ngenerate = [\"Ac.hpp\"]\n",
    );
    let list = dir.path().join("names.txt");

    let out = Command::new(bin())
        .args([
            "--rules".as_ref(),
            rules.as_os_str(),
            "-d".as_ref(),
            dir.path().as_os_str(),
            "--filenames".as_ref(),
            list.as_os_str(),
            "--".as_ref(),
            model.as_os_str(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(fs::read_to_string(&list).unwrap(), "");
}

/// The shipped presets are the artifact users copy, so they must at least load.
/// `tests/filenames.rs` holds their *output* to upstream `fpp-filenames`.
#[test]
fn every_shipped_preset_loads() {
    let dir = TempDir::new("presets");
    let model = dir.write("m.fpp", MODEL);
    let presets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("presets");

    let mut found = 0;
    for entry in fs::read_dir(&presets).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "toml") {
            continue;
        }
        found += 1;
        let out = Command::new(bin())
            .args([
                "--rules".as_ref(),
                path.as_os_str(),
                "-d".as_ref(),
                dir.path().as_os_str(),
                "--".as_ref(),
                model.as_os_str(),
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{} failed: {}",
            path.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert_eq!(
        found, 7,
        "one preset per upstream mode, plus the expanded autocode"
    );
}

/// Expansion adds each internal state machine's implicit `enum State`, so a plain
/// `DefEnum` group finds it. Without `expand` the enum does not exist in the model.
#[test]
fn expand_makes_the_implicit_state_enum_selectable() {
    let dir = TempDir::new("expand");
    let group = "[[group]]\nnode = \"DefEnum\"\ngenerate = [\"EnumAc.hpp\"]\n";

    let off = rules(&dir, group);
    assert_eq!(names(&off, dir.path()), Vec::<String>::new());

    let on = rules(&dir, &format!("expand = true\n\n{group}"));
    assert_eq!(names(&on, dir.path()), ["C_SM_StateEnumAc.hpp"]);
}

/// An external (bodyless) state machine defines no states, so it gets no enum —
/// matching `fpp-to-cpp`, which generates nothing for one.
#[test]
fn expand_skips_an_external_state_machine() {
    let dir = TempDir::new("expandexternal");
    let model = dir.write(
        "sm.fpp",
        "module M {\n  state machine External\n  state machine Bodied {\n    initial enter S\n    state S\n  }\n}\n",
    );
    let rules = dir.write(
        "rules.toml",
        "expand = true\n\n[[group]]\nnode = \"DefEnum\"\ngenerate = [\"EnumAc.hpp\"]\n",
    );
    let out = Command::new(bin())
        .args([
            "--rules".as_ref(),
            rules.as_os_str(),
            "-d".as_ref(),
            dir.path().as_os_str(),
            "--".as_ref(),
            model.as_os_str(),
        ])
        .output()
        .unwrap();
    assert_eq!(names(&out, dir.path()), ["Bodied_StateEnumAc.hpp"]);
}

/// `--json` promises to dump exactly what a query sees, so passing `--rules` must
/// make it report the model those rules read.
#[test]
fn expand_reaches_the_json_dump() {
    let dir = TempDir::new("expandjson");
    let model = dir.write("m.fpp", MODEL);
    let dump = |expand: bool| {
        let rules = dir.write(
            "rules.toml",
            &format!(
                "{}[[group]]\nnode = \"DefEnum\"\ngenerate = [\"EnumAc.hpp\"]\n",
                if expand { "expand = true\n\n" } else { "" }
            ),
        );
        let out = run(&[
            "--json",
            "--rules",
            &rules.to_string_lossy(),
            &model.to_string_lossy(),
        ]);
        assert!(out.status.success(), "{}", stderr(&out));
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    assert!(!dump(false).contains("__FPRIME_UNINITIALIZED"));
    assert!(dump(true).contains("__FPRIME_UNINITIALIZED"));
}

/// TOML puts a bare key under the table above it, so `expand` written after the
/// first `[[group]]` is silently a group key. Saying so beats reporting a
/// correctly-spelled setting as an unknown field.
#[test]
fn expand_below_a_group_says_where_it_belongs() {
    let dir = TempDir::new("expandmisplaced");
    let out = rules(
        &dir,
        r#"[[group]]
node = "DefEnum"
generate = ["EnumAc.hpp"]
expand = true
"#,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    assert!(text.contains("file-wide setting"), "{text}");
    assert!(text.contains("above the first `[[group]]`"), "{text}");
    assert!(text.contains("rules.toml:4:"), "{text}");
}
