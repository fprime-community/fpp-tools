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
/// This covers what reading the file can decide: the file-wide `expand`, the kind,
/// the suffixes, and whether a query parses. Whether a query *evaluates* needs a
/// node to evaluate it against, so an unknown field surfaces later, during the walk.
#[test]
fn every_bad_group_is_reported_in_one_run() {
    let dir = TempDir::new("allerrors");
    let out = rules(
        &dir,
        r#"expand = ["templats"]

[[group]]
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
    assert!(text.contains("is not an expansion"), "{text}");
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

/// One definition on each expansion's axis, so a single rules file reads both.
///
/// The only `component` outside the template body is the one an expansion
/// contributes, and the only `enum` anywhere is the one state-enum synthesis would
/// add, so a `DefComponent` group and a `DefEnum` group each match exactly when
/// their own expansion ran.
const TEMPLATE_AND_STATE_MACHINE: &str = "\
module template T {
  passive component FromTemplate { }
}
module M {
  expand T
  state machine SM {
    initial enter S1
    state S1
  }
}
";

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

/// Every spelling of `expand`, and the pair of expansions it names.
///
/// The two axes are independent, which is what the list form buys: `templates` alone
/// leaves each state machine without its implicit `State`, and `state-enums` alone
/// leaves a template body where it was written. `true` is both, and the two names
/// spell that out in either order, since a set has no order.
#[test]
fn every_expand_spelling_selects_its_own_pair() {
    let dir = TempDir::new("expandspellings");
    let groups = "\
[[group]]
node = \"DefComponent\"
generate = [\"ComponentAc.hpp\"]

[[group]]
node = \"DefEnum\"
generate = [\"EnumAc.hpp\"]
";
    let from_template = "FromTemplateComponentAc.hpp";
    let state_enum = "SM_StateEnumAc.hpp";

    for (setting, want) in [
        // Absent, `false`, and the empty list are all "the model as parsed".
        ("", vec![]),
        ("expand = false\n\n", vec![]),
        ("expand = []\n\n", vec![]),
        ("expand = [\"templates\"]\n\n", vec![from_template]),
        ("expand = [\"state-enums\"]\n\n", vec![state_enum]),
        ("expand = true\n\n", vec![from_template, state_enum]),
        (
            "expand = [\"templates\", \"state-enums\"]\n\n",
            vec![from_template, state_enum],
        ),
        (
            "expand = [\"state-enums\", \"templates\"]\n\n",
            vec![from_template, state_enum],
        ),
    ] {
        let out = query(
            &dir,
            &format!("{setting}{groups}"),
            TEMPLATE_AND_STATE_MACHINE,
        );
        assert_eq!(
            names(&out, dir.path()),
            want,
            "for `{}`",
            setting.trim_end()
        );
    }
}

/// Without the `templates` expansion a query reads the model exactly as parsed: the
/// definitions the source writes, and only those.
///
/// A template body is not part of the model wherever it is written, so the component
/// it defines is absent even though the file expands the template — the answer a
/// tool reporting on the source itself wants, rather than on what an autocoder
/// compiles.
#[test]
fn the_unexpanded_model_holds_only_what_the_parser_produced() {
    let dir = TempDir::new("aswritten");
    let model = "\
module template T {
  passive component FromTemplate { }
}
module M {
  passive component AsWritten { }
  expand T
}
";
    let group = "[[group]]\nnode = \"DefComponent\"\ngenerate = [\"ComponentAc.hpp\"]\n";

    let off = query(&dir, &format!("expand = false\n\n{group}"), model);
    assert_eq!(names(&off, dir.path()), ["AsWrittenComponentAc.hpp"]);

    // ...and with the expansion on, the same rules reach both.
    let on = query(&dir, &format!("expand = [\"templates\"]\n\n{group}"), model);
    assert_eq!(
        names(&on, dir.path()),
        ["AsWrittenComponentAc.hpp", "FromTemplateComponentAc.hpp"]
    );
}

/// An expansion names its template the way any qualified identifier names a
/// definition: a relative name resolves in the innermost enclosing scope that has
/// it, and an absolute name (`.T`) in the global scope only.
#[test]
fn an_absolute_template_name_skips_the_enclosing_scopes() {
    let dir = TempDir::new("absolute");
    let model = "\
module template T {
  passive component Global { }
}
module M {
  module template T {
    passive component Nested { }
  }
  module N {
    expand .T
  }
  module O {
    expand T
  }
}
";
    let out = query(
        &dir,
        "expand = [\"templates\"]\n\n[[group]]\nnode = \"DefComponent\"\ngenerate = [\"ComponentAc.hpp\"]\n",
        model,
    );
    assert_eq!(
        names(&out, dir.path()),
        ["GlobalComponentAc.hpp", "NestedComponentAc.hpp"]
    );
}

/// A state machine an expansion contributes gets its implicit `State` like a
/// hand-written one, so the two expansions cannot be run in either order: whichever
/// runs first must leave its output where the other still reaches it.
#[test]
fn a_state_machine_from_an_expansion_gets_its_state_enum() {
    let dir = TempDir::new("expandorder");
    let out = query(
        &dir,
        "\
expand = true

[[group]]
node = \"DefStateMachine\"
where = '$.members != null'
generate = [\"StateMachineAc.hpp\"]

[[group]]
node = \"DefEnum\"
generate = [\"EnumAc.hpp\"]
",
        "\
module template T {
  state machine Contributed {
    initial enter S1
    state S1
  }
}
module M {
  expand T
}
",
    );
    assert_eq!(
        names(&out, dir.path()),
        [
            "ContributedStateMachineAc.hpp",
            "Contributed_StateEnumAc.hpp"
        ],
        "the state machine's own pair shows the expansion was materialized, so a \
         missing `State` is about the order the two expansions run in"
    );
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
/// make it report the model those rules read — on each axis separately, since a dump
/// that showed one expansion and hid the other would describe a model no query is
/// ever run against.
#[test]
fn expand_reaches_the_json_dump() {
    let dir = TempDir::new("expandjson");
    let model = dir.write("m.fpp", TEMPLATE_AND_STATE_MACHINE);
    let dump = |setting: &str| {
        let rules = dir.write(
            "rules.toml",
            &format!("{setting}[[group]]\nnode = \"DefEnum\"\ngenerate = [\"EnumAc.hpp\"]\n"),
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

    /// The first object under `key`, at any depth.
    fn find<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
        match value {
            serde_json::Value::Object(map) => map
                .get(key)
                .or_else(|| map.values().find_map(|v| find(v, key))),
            serde_json::Value::Array(items) => items.iter().find_map(|v| find(v, key)),
            _ => None,
        }
    }

    // An expansion specifier's `members` are `null` until template expansion fills
    // them in, which is how the dump tells the two models apart: the template body
    // itself is serialized either way.
    let expanded = |dump: &str| {
        let value: serde_json::Value = serde_json::from_str(dump).unwrap();
        find(&value, "SpecTemplateExpand")
            .and_then(|spec| spec.get("members"))
            .expect("a SpecTemplateExpand with a `members` field")
            != &serde_json::Value::Null
    };
    // The first constant of a synthesized `State`, which nothing else in the model
    // writes.
    let state_enum = |dump: &str| dump.contains("__FPRIME_UNINITIALIZED");

    let none = dump("");
    assert!(!state_enum(&none));
    assert!(!expanded(&none));

    let templates = dump("expand = [\"templates\"]\n\n");
    assert!(!state_enum(&templates));
    assert!(expanded(&templates));

    let enums = dump("expand = [\"state-enums\"]\n\n");
    assert!(state_enum(&enums));
    assert!(!expanded(&enums));

    let all = dump("expand = true\n\n");
    assert!(state_enum(&all));
    assert!(expanded(&all));
}

/// A misspelled expansion is carated at the name that is wrong, and the note lists
/// the names that exist rather than guessing which of them was meant.
#[test]
fn an_unknown_expansion_names_the_real_ones() {
    let dir = TempDir::new("badexpansion");
    let out = rules(
        &dir,
        r#"expand = ["templats"]

[[group]]
node = "DefEnum"
generate = ["EnumAc.hpp"]
"#,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    assert!(text.contains("`templats` is not an expansion"), "{text}");
    assert!(
        text.contains("the expansions are `templates`, `state-enums`"),
        "{text}"
    );
    assert!(
        text.contains("`expand = true` is all of them, `expand = false` none"),
        "{text}"
    );
    // The caret covers the element rather than the whole list, so it starts at the
    // name's own column and is eight wide.
    assert!(text.contains("rules.toml:1:12"), "{text}");
    assert!(text.contains("^^^^^^^^ `templats`"), "{text}");
}

/// One spelling per meaning: a bare name is not a one-element list, and the note
/// shows the list form instead of leaving the author to guess where the brackets go.
///
/// The form it shows is built from the expansions that exist, not from the name the
/// file wrote — an unchecked name echoed back would suggest a list of it, and the
/// suggestion has to be one worth following whether or not the name is spelled right.
#[test]
fn a_bare_expansion_name_is_rejected_with_the_list_spelling() {
    let dir = TempDir::new("bareexpansion");
    let out = rules(
        &dir,
        r#"expand = "templates"

[[group]]
node = "DefEnum"
generate = ["EnumAc.hpp"]
"#,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    assert!(
        text.contains("`expand` is a list of expansions, not one name"),
        "{text}"
    );
    assert!(
        text.contains("write it as a list: `expand = [\"templates\", \"state-enums\"]`"),
        "{text}"
    );
    // The complaint is about the value's shape, so the caret covers the value with
    // its quotes: column 10, eleven wide.
    assert!(text.contains("rules.toml:1:10"), "{text}");
    assert!(text.contains("^^^^^^^^^^^ `expand` is a list"), "{text}");

    // ...and a value carrying TOML's own quoting gets the same note, so what the note
    // suggests is a list TOML reads whatever the file wrote.
    let quoted = rules(
        &dir,
        "expand = 'a\"b'\n\n[[group]]\nnode = \"DefEnum\"\ngenerate = [\"EnumAc.hpp\"]\n",
    );
    assert_eq!(quoted.status.code(), Some(1));
    let text = stderr(&quoted);
    assert!(
        text.contains("write it as a list: `expand = [\"templates\", \"state-enums\"]`"),
        "{text}"
    );
}

/// Every bad name in one list is reported, each carated at the element that wrote
/// it, so fixing a list takes one round trip however many names it misspells.
#[test]
fn every_bad_expansion_name_reports_at_its_own_element() {
    let dir = TempDir::new("badexpansions");
    for (list, second) in [
        (r#"["templats", "state-enum"]"#, "state-enum"),
        // A name written twice is still two mistakes, and the second report must
        // point at the second element rather than back at the first.
        (r#"["templats", "templats"]"#, "templats"),
    ] {
        let out = rules(
            &dir,
            &format!(
                "expand = {list}\n\n[[group]]\nnode = \"DefEnum\"\ngenerate = [\"EnumAc.hpp\"]\n"
            ),
        );
        assert_eq!(out.status.code(), Some(1), "for `{list}`");
        let text = stderr(&out);
        assert_eq!(
            text.matches("is not an expansion").count(),
            2,
            "for `{list}`: {text}"
        );
        assert!(
            text.contains("`templats` is not an expansion"),
            "for `{list}`: {text}"
        );
        assert!(
            text.contains(&format!("`{second}` is not an expansion")),
            "for `{list}`: {text}"
        );
        // The two elements of `expand = [...]` on line 1 start at columns 12 and 24.
        assert!(text.contains("rules.toml:1:12"), "for `{list}`: {text}");
        assert!(text.contains("rules.toml:1:24"), "for `{list}`: {text}");
    }
}

/// Two independent expansions read like two settings, so the key invites being
/// written once per expansion. TOML takes the second as a redefinition of the first,
/// and the report names that: it carries the same span as a key written below a
/// group, whose advice is to move a key that is already where it belongs.
#[test]
fn expand_written_twice_says_the_names_go_in_one_list() {
    let dir = TempDir::new("expandtwice");
    let out = rules(
        &dir,
        r#"expand = ["templates"]
expand = ["state-enums"]

[[group]]
node = "DefEnum"
generate = ["EnumAc.hpp"]
"#,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    assert!(
        text.contains("`expand` is written more than once"),
        "{text}"
    );
    // The note spells the one list that asks for both.
    assert!(
        text.contains("`expand = [\"templates\", \"state-enums\"]`"),
        "{text}"
    );
    // Reported at the second spelling of the key, and not as a misplaced one.
    assert!(text.contains("rules.toml:2:"), "{text}");
    assert!(!text.contains("file-wide setting"), "{text}");
}

/// A file with no group at all still has its `expand` read, so a file that gets both
/// wrong reports both rather than handing them back one run at a time.
#[test]
fn a_bad_expansion_is_reported_with_no_groups_to_check() {
    let dir = TempDir::new("expandnogroups");
    let out = rules(&dir, "expand = [\"templats\"]\n");
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    assert!(text.contains("`templats` is not an expansion"), "{text}");
    assert!(text.contains("defines no `[[group]]`"), "{text}");
}

/// An empty element cannot be quoted into a headline, so it is reported as the
/// element it is: the list still gets one report per bad name.
#[test]
fn an_empty_expansion_name_is_reported_as_the_element() {
    let dir = TempDir::new("emptyexpansion");
    let out = rules(
        &dir,
        r#"expand = ["", "templat"]

[[group]]
node = "DefEnum"
generate = ["EnumAc.hpp"]
"#,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    assert!(text.contains("this expansion name is empty"), "{text}");
    assert!(!text.contains("`` is not an expansion"), "{text}");
    assert!(text.contains("`templat` is not an expansion"), "{text}");
    // The empty element's own quotes are carated, at column 11 and two wide, and the
    // name after it at its own column rather than anywhere near the first.
    assert!(text.contains("rules.toml:1:11"), "{text}");
    assert!(text.contains("^^ this expansion name is empty"), "{text}");
    assert!(text.contains("rules.toml:1:16"), "{text}");
}

/// A misspelling that reads as part of a name the file spelled right is carated at
/// the element that wrote it, not at the one it is a substring of.
#[test]
fn a_bad_name_is_carated_past_a_good_one_it_reads_inside_of() {
    let dir = TempDir::new("substringexpansion");
    for (list, bad, column) in [
        (r#"["templates", "template"]"#, "template", "1:25"),
        (r#"["state-enums", "enums"]"#, "enums", "1:27"),
    ] {
        let out = rules(
            &dir,
            &format!(
                "expand = {list}\n\n[[group]]\nnode = \"DefEnum\"\ngenerate = [\"EnumAc.hpp\"]\n"
            ),
        );
        assert_eq!(out.status.code(), Some(1), "for `{list}`");
        let text = stderr(&out);
        assert_eq!(
            text.matches("is not an expansion").count(),
            1,
            "for `{list}`: {text}"
        );
        assert!(
            text.contains(&format!("`{bad}` is not an expansion")),
            "for `{list}`: {text}"
        );
        assert!(
            text.contains(&format!("rules.toml:{column}")),
            "for `{list}`: {text}"
        );
    }
}

/// TOML puts a bare key under the table above it, so `expand` written after the
/// first `[[group]]` is silently a group key. Saying so beats reporting a
/// correctly-spelled setting as an unknown field — whatever the value is, since the
/// misplaced key is what identifies the mistake and the value never gets read.
#[test]
fn expand_below_a_group_says_where_it_belongs() {
    let dir = TempDir::new("expandmisplaced");
    for value in ["true", "false", r#"["templates"]"#, "[]", r#""templates""#] {
        let out = rules(
            &dir,
            &format!(
                "[[group]]\nnode = \"DefEnum\"\ngenerate = [\"EnumAc.hpp\"]\nexpand = {value}\n"
            ),
        );
        assert_eq!(out.status.code(), Some(1), "for `{value}`");
        let text = stderr(&out);
        assert!(text.contains("file-wide setting"), "for `{value}`: {text}");
        assert!(
            text.contains("above the first `[[group]]`"),
            "for `{value}`: {text}"
        );
        assert!(text.contains("rules.toml:4:"), "for `{value}`: {text}");
    }
}
