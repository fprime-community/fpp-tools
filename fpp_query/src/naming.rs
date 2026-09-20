use fpp_core::{Diagnostic, Span};

/// `SpecPortInstance` is an `#[ast]` union the walk passes straight through, so a
/// visitor never sees it. Naming it would silently match nothing.
const TRANSPARENT_KINDS: &[(&str, &[&str])] = &[(
    "SpecPortInstance",
    &["SpecGeneralPortInstance", "SpecSpecialPortInstance"],
)];

/// True when `kind` can be matched by the walk.
pub fn kind_is_selectable(kind: &str) -> bool {
    fpp_ast::Node::KIND_NAMES.contains(&kind) && !TRANSPARENT_KINDS.iter().any(|(k, _)| *k == kind)
}

/// Why `kind` cannot be selected: the headline, then any notes.
fn kind_problem(kind: &str) -> (String, Vec<String>) {
    if let Some((transparent, concrete)) = TRANSPARENT_KINDS.iter().find(|(k, _)| *k == kind) {
        return (
            format!("`{transparent}` is a transparent wrapper"),
            vec![format!(
                "the walk passes through it, so it never matches; use {}",
                concrete
                    .iter()
                    .map(|c| format!("`{c}`"))
                    .collect::<Vec<_>>()
                    .join(" or ")
            )],
        );
    }
    (
        format!("`{kind}` is not an AST node kind"),
        vec!["`fpp-query --fields` lists every kind".to_string()],
    )
}

/// The first character that cannot appear in a generated filename, if any.
///
/// Both halves of `<stem><suffix>` are held to this: every emitted line becomes an
/// `add_custom_command(OUTPUT ...)` entry, so a newline splits it into two entries
/// (one of them blank), a `;` splits it into two CMake list elements, and a path
/// separator escapes the flat `-d` directory the output layout assumes.
pub fn bad_filename_char(name: &str) -> Option<char> {
    name.chars()
        .find(|c| matches!(c, '\n' | '\r' | ';' | '/' | '\\'))
}

/// Why `suffix` cannot be appended to a stem, if it cannot.
fn suffix_problem(suffix: &str) -> Option<(String, Vec<String>)> {
    if suffix.is_empty() {
        return Some(("a generated suffix cannot be empty".to_string(), Vec::new()));
    }
    bad_filename_char(suffix).map(|bad| {
        (
            format!("`{suffix}` contains {bad:?}, which cannot appear in a generated filename"),
            vec!["generated files are written flat into the `-d` directory".to_string()],
        )
    })
}

fn as_diagnostic(span: Span, (headline, notes): (String, Vec<String>)) -> Diagnostic {
    notes
        .into_iter()
        .fold(span.error(headline), Diagnostic::note)
}

/// Used for a `node` in a rules file and for a `$^Kind` path root.
pub fn unknown_kind(span: Span, kind: &str) -> Diagnostic {
    as_diagnostic(span, kind_problem(kind))
}

pub fn check_suffix_at(span: Span, suffix: &str) -> Result<(), Diagnostic> {
    match suffix_problem(suffix) {
        None => Ok(()),
        Some(problem) => Err(as_diagnostic(span, problem)),
    }
}
