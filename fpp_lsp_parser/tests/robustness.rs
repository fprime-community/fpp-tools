//! Robustness checks: the lossless parser never panics and always reproduces
//! its input, whatever token appears in whatever position.

use fpp_lsp_parser::{SyntaxKind, TopEntryPoint, parse};
use std::panic;

/// The source spelling of every keyword.
fn keywords() -> Vec<String> {
    (0..SyntaxKind::ROOT as u16)
        .map(SyntaxKind::from)
        .filter(|k| k.is_keyword())
        .map(|k| {
            let name = format!("{:?}", k);
            let name = name.strip_suffix("_KW").unwrap();
            if k.is_type_primitive_keyword()
                && !matches!(k, SyntaxKind::BOOL_KW | SyntaxKind::STRING_KW)
            {
                name.to_string()
            } else {
                name.to_ascii_lowercase()
            }
        })
        .collect()
}

/// Token sequences to place at each position.
fn fillers() -> Vec<String> {
    let mut words: Vec<String> = keywords();
    words.extend(
        [
            "x",
            "1",
            "1.5",
            "\"s\"",
            "\"\"\"s\"\"\"",
            ".",
            ":",
            ",",
            ";",
            "=",
            "(",
            ")",
            "{",
            "}",
            "[",
            "]",
            "->",
            "-",
            "+",
            "*",
            "/",
            "<<",
            ">>",
            "@ pre\n",
            "@< post\n",
            "# c\n",
            "\n",
            "\t",
            "$",
            "<",
            "\\\n",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    let mut out = vec![String::new()];
    for w in &words {
        out.push(w.clone());
        out.push(format!("{w} x"));
        out.push(format!("{w} }}"));
        out.push(format!("{w} {w}"));
    }
    out
}

/// Each context is an entry point and a template with `$` marking the hole.
const CONTEXTS: &[(TopEntryPoint, &str)] = &[
    (TopEntryPoint::Module, "$"),
    (TopEntryPoint::Module, "module M { $ }"),
    (TopEntryPoint::Module, "passive component C { $ }"),
    (TopEntryPoint::Module, "interface I { $ }"),
    (TopEntryPoint::Module, "topology T { $ }"),
    (TopEntryPoint::Module, "state machine S { $ }"),
    (TopEntryPoint::Module, "state machine S { state A { $ } }"),
    (
        TopEntryPoint::Module,
        "topology T { telemetry packets P { $ } }",
    ),
    (
        TopEntryPoint::Module,
        "topology T { telemetry packets P { packet A group 0 { $ } } }",
    ),
    (TopEntryPoint::Module, "constant a = $"),
    (TopEntryPoint::Module, "constant a = ($"),
    (TopEntryPoint::Module, "type T = $"),
    (TopEntryPoint::Module, "array A = [$] U32"),
    (TopEntryPoint::Module, "port P($)"),
    (TopEntryPoint::Module, "enum E { $ }"),
    (TopEntryPoint::Module, "struct S { $ }"),
    (TopEntryPoint::Module, "topology T { connections C { $ } }"),
    (TopEntryPoint::Module, "module template T($) { }"),
    (TopEntryPoint::Module, "expand T($)"),
    (TopEntryPoint::Module, "locate $"),
    (TopEntryPoint::Component, "$"),
    (TopEntryPoint::Topology, "$"),
];

#[test]
fn parser_never_panics_and_is_lossless() {
    let fillers = fillers();
    let mut failures = Vec::new();
    for (entry, template) in CONTEXTS {
        for filler in &fillers {
            let text = template.replace('$', filler);
            let result = panic::catch_unwind(|| parse(&text, *entry));
            match result {
                Err(_) => failures.push(format!("panic: {:?} {:?}", entry, text)),
                Ok(p) => {
                    if p.syntax_node().text().to_string() != text {
                        failures.push(format!("lossy: {:?} {:?}", entry, text));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
