//! The query language and the filename stem, end to end through the binary.
//!
//! Every case is a rules file, because that is the only way to give the tool a
//! group. What is under test here is what a query *means*; `rules.rs` covers the
//! file's own schema and diagnostics, and `cli.rs` the process contract.

mod common;

use common::{MODEL, TempDir, group, names, stderr};

#[test]
fn stem_prefixes_component_and_state_machine_but_not_module() {
    let dir = TempDir::new("stem");
    let out = group(
        &dir,
        "node = \"DefArray\"\ngenerate = [\"ArrayAc.hpp\", \"ArrayAc.cpp\"]",
        MODEL,
    );
    // `module M` contributes nothing; a component and a state machine each
    // contribute a `Parent_` prefix, recursively, matching fpp-to-cpp.
    assert_eq!(
        names(&out, dir.path()),
        [
            "AArrayAc.cpp",
            "AArrayAc.hpp",
            "C_AArrayAc.cpp",
            "C_AArrayAc.hpp",
            "C_SM_AArrayAc.cpp",
            "C_SM_AArrayAc.hpp",
        ]
    );
}

#[test]
fn matches_only_the_deployment_topology() {
    let dir = TempDir::new("deployment");
    let out = group(
        &dir,
        "node = \"DefTopology\"\nwhere = '$.is_deployment'\ngenerate = [\"TopologyAc.cpp\"]",
        MODEL,
    );
    // `topology T2` is not a deployment.
    assert_eq!(names(&out, dir.path()), ["T1TopologyAc.cpp"]);
}

/// A predicate over the annotation lines, which live in the compiler context rather
/// than in the node's fields.
#[test]
fn annotations_are_queryable_three_ways() {
    let dir = TempDir::new("annot");
    let select = |where_: &str| {
        let out = group(
            &dir,
            &format!("node = \"DefComponent\"\nwhere = '{where_}'\ngenerate = [\"Ac.cpp\"]"),
            MODEL,
        );
        names(&out, dir.path())
    };

    // Substring of any line, exact whole line, and "has any annotation at all".
    assert_eq!(select(r#"$@ contains "static-tlm""#), ["CAc.cpp"]);
    assert_eq!(select(r#""static-tlm-packetizer" in $@"#), ["CAc.cpp"]);
    assert_eq!(select("len($@) > 0"), ["CAc.cpp"]);
    assert_eq!(select(r#"$@pre matches "static-*""#), ["CAc.cpp"]);
    // And the negations select the unannotated component instead.
    assert_eq!(select("len($@) == 0"), ["ActAc.cpp"]);
    assert_eq!(select(r#"!($@ contains "static-tlm")"#), ["ActAc.cpp"]);
}

#[test]
fn leaf_enum_compares_against_its_variant_name() {
    let dir = TempDir::new("enumkind");
    let out = group(
        &dir,
        "node = \"DefComponent\"\nwhere = '$.kind in [\"Active\", \"Queued\"]'\ngenerate = [\"Ac.cpp\"]",
        MODEL,
    );
    assert_eq!(names(&out, dir.path()), ["ActAc.cpp"]);
}

#[test]
fn ancestor_root_reaches_the_enclosing_definition() {
    let dir = TempDir::new("ancestor");
    let out = group(
        &dir,
        "node = \"DefArray\"\nwhere = '$^DefStateMachine != null'\ngenerate = [\"Ac.cpp\"]",
        MODEL,
    );
    assert_eq!(names(&out, dir.path()), ["C_SM_AAc.cpp"]);
}

/// An external state machine has no body and generates nothing upstream, so the
/// absent `members` option has to be distinguishable from an empty one.
#[test]
fn absent_option_distinguishes_external_state_machines() {
    let dir = TempDir::new("external");
    let out = group(
        &dir,
        "node = \"DefStateMachine\"\nwhere = '$.members != null'\ngenerate = [\"StateMachineAc.hpp\"]",
        "module M {\n  state machine External\n  state machine Bodied {\n    initial enter S\n    state S\n  }\n}\n",
    );
    assert_eq!(names(&out, dir.path()), ["BodiedStateMachineAc.hpp"]);
}

/// Many definitions collapsing onto one stem must dedup, not repeat: a duplicate
/// `add_custom_command(OUTPUT ...)` is a build error.
#[test]
fn name_expression_overrides_the_stem() {
    let dir = TempDir::new("name");
    let out = group(
        &dir,
        "node = \"DefArray\"\nname = '\"FppConstants\"'\ngenerate = [\"Ac.hpp\", \"Ac.cpp\"]",
        MODEL,
    );
    assert_eq!(
        names(&out, dir.path()),
        ["FppConstantsAc.cpp", "FppConstantsAc.hpp"]
    );
}

#[test]
fn unknown_field_names_the_alternatives() {
    let dir = TempDir::new("badfield");
    let out = group(
        &dir,
        "node = \"DefTopology\"\nwhere = '$.is_deploy'\ngenerate = [\"TopologyAc.hpp\"]",
        MODEL,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = stderr(&out);
    assert!(text.contains("has no field `is_deploy`"), "{text}");
    // The real field names, rather than a guess at which was meant.
    assert!(text.contains("is_deployment"), "{text}");
}

#[test]
fn definitions_spliced_in_by_include_are_found() {
    let dir = TempDir::new("include");
    dir.write("frag.fppi", "array Inc = [2] U8\n");
    let out = group(
        &dir,
        "node = \"DefArray\"\ngenerate = [\"ArrayAc.hpp\"]",
        "module M {\n  include \"frag.fppi\"\n}\n",
    );
    assert_eq!(names(&out, dir.path()), ["IncArrayAc.hpp"]);
}

#[test]
fn included_metadata_distinguishes_spliced_definitions() {
    let dir = TempDir::new("included");
    dir.write("frag.fppi", "array Inc = [2] U8\n");
    let model = "module M {\n  array Own = [1] U8\n  include \"frag.fppi\"\n}\n";
    let select = |where_: &str| {
        let out = group(
            &dir,
            &format!("node = \"DefArray\"\nwhere = '{where_}'\ngenerate = [\"Ac.hpp\"]"),
            model,
        );
        names(&out, dir.path())
    };
    assert_eq!(select("$@included"), ["IncAc.hpp"]);
    assert_eq!(select("!$@included"), ["OwnAc.hpp"]);
}

/// A stem is model-derived, so these are reachable from the FPP source, not only
/// from a hand-written expression.
#[test]
fn a_stem_that_would_corrupt_the_list_is_rejected() {
    let dir = TempDir::new("stemchars");
    let model = "module M {\n@ tag a/b;c\npassive component C { }\n}\n";

    // A `/` or `;` straight out of an annotation; an embedded newline; and an
    // absolute path, which `Path::join` would let silently replace `-d`.
    for name in [
        "$@pre[0]",
        r#""a" + "\n" + "b""#,
        r#""a;b""#,
        r#""../../etc/pwn""#,
        "$@file",
    ] {
        let out = group(
            &dir,
            &format!("node = \"DefComponent\"\nname = '{name}'\ngenerate = [\".hpp\"]"),
            model,
        );
        assert_eq!(out.status.code(), Some(1), "name = '{name}' was accepted");
        assert!(
            stderr(&out).contains("cannot appear in a filename"),
            "name = '{name}': {}",
            stderr(&out)
        );
    }
}

#[test]
fn a_kind_that_names_nothing_requires_an_explicit_name() {
    let dir = TempDir::new("nameless");
    let model = "module M {\n  topology T {\n    connections C { a.b -> c.d }\n  }\n}\n";

    // Without a stem the emitted path would be a bare `<dir>/<suffix>` that every
    // match of the group collapses onto.
    let bare = group(&dir, "node = \"Connection\"\ngenerate = [\".hpp\"]", model);
    assert_eq!(bare.status.code(), Some(1));
    assert!(
        stderr(&bare).contains("introduces no name"),
        "{}",
        stderr(&bare)
    );

    let named = group(
        &dir,
        "node = \"Connection\"\nname = '\"conn\"'\ngenerate = [\".hpp\"]",
        model,
    );
    assert_eq!(names(&named, dir.path()), ["conn.hpp"]);
}

/// A malformed query must produce a diagnostic, never a panic or a stack overflow:
/// either of those skips the code that writes the output file, so CMake reports its
/// own `file(STRINGS)` failure instead of our error.
#[test]
fn pathological_queries_diagnose_instead_of_crashing() {
    let dir = TempDir::new("pathological");

    let deep_parens = format!("{}$.is_deployment{}", "(".repeat(3000), ")".repeat(3000));
    let long_chain = vec!["$.is_deployment"; 3000].join(" || ");
    let deep_not = format!("{}$.is_deployment", "!".repeat(3000));
    // An escape before a multi-byte character: the span must not end mid-character,
    // or the diagnostic renderer panics.
    let bad_escape = "$.name == \"\\\u{e9}\"".to_string();

    for where_ in [deep_parens, long_chain, deep_not, bad_escape] {
        let out = group(
            &dir,
            &format!("node = \"DefTopology\"\nwhere = '{where_}'\ngenerate = [\".hpp\"]"),
            MODEL,
        );
        assert_eq!(
            out.status.code(),
            Some(1),
            "expected a diagnosed failure, got {:?} for a {}-char query",
            out.status,
            where_.len()
        );
        assert!(!out.stderr.is_empty(), "a diagnostic must be printed");
    }

    // A reasonable amount of nesting is still accepted.
    let ok = format!("{}$.is_deployment{}", "(".repeat(20), ")".repeat(20));
    let out = group(
        &dir,
        &format!("node = \"DefTopology\"\nwhere = '{ok}'\ngenerate = [\"Ac.cpp\"]"),
        MODEL,
    );
    assert_eq!(names(&out, dir.path()), ["T1Ac.cpp"]);
}

#[test]
fn multi_byte_query_text_is_handled() {
    let dir = TempDir::new("utf8");
    let model = "module M {\n@ h\u{e9}llo w\u{f6}rld\npassive component C { }\n}\n";

    let out = group(
        &dir,
        "node = \"DefComponent\"\nwhere = '$@ contains \"w\u{f6}rld\"'\ngenerate = [\"Ac.cpp\"]",
        model,
    );
    assert_eq!(names(&out, dir.path()), ["CAc.cpp"]);

    // A stray multi-byte character is reported, not panicked on.
    let bad = group(
        &dir,
        "node = \"DefComponent\"\nwhere = '$.name \u{2260} \"x\"'\ngenerate = [\"Ac.cpp\"]",
        model,
    );
    assert_eq!(bad.status.code(), Some(1));
    assert!(
        stderr(&bad).contains("unexpected character"),
        "{}",
        stderr(&bad)
    );
}

/// `?` is a glob metacharacter inside `matches`, not an operator of its own.
#[test]
fn a_bare_question_mark_is_not_an_operator() {
    let dir = TempDir::new("noqmark");

    let stray = group(
        &dir,
        "node = \"DefTopology\"\nwhere = '$.is_deployment ? true'\ngenerate = [\"Ac.cpp\"]",
        MODEL,
    );
    assert_eq!(stray.status.code(), Some(1));
    assert!(
        stderr(&stray).contains("unexpected character"),
        "{}",
        stderr(&stray)
    );

    // ...while it is an ordinary single-character wildcard in a glob.
    let glob = group(
        &dir,
        "node = \"DefTopology\"\nwhere = '$.name matches \"T?\"'\ngenerate = [\"Ac.cpp\"]",
        MODEL,
    );
    assert_eq!(names(&glob, dir.path()), ["T1Ac.cpp", "T2Ac.cpp"]);
}
