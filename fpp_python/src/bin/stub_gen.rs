//! Dumps `python/fpp_python/__init__.pyi` from the pyclasses annotated with
//! `#[gen_stub_pyclass]` / `#[gen_stub_pymethods]`, via pyo3-stub-gen's inventory.
//!
//! Build/run WITHOUT the `extension-module` feature (a standalone executable
//! must link libpython):
//!   cargo run -p fpp_python --no-default-features --features stubgen --bin stub_gen
//!
//! The output path is resolved from this crate's `pyproject.toml`
//! (`module-name = fpp_python`, pure-Rust layout), so the stub lands beside
//! `Cargo.toml` as `fpp_python.pyi`. maturin ships it in the wheel as
//! `fpp_python/__init__.pyi`. See `fpp_python::stub_info`.
//!
//! After generation we inject the closed-union type aliases
//! (`Value = IntegerValue | … `), which pyo3-stub-gen cannot express as named
//! `TypeAlias`es — the union return sites render as the alias name via a custom
//! `PyStubType`, and these lines define it (see `fpp_python::union_aliases`).
//!
//! Two fixups follow, both because the aliases are injected *after* generation
//! and so are invisible to the generator: their names are added to the emitted
//! `__all__`, and the doubled spacing pyo3-stub-gen leaves around union `|` is
//! collapsed.

use std::fs;
use std::path::Path;

fn main() -> pyo3_stub_gen::Result<()> {
    fpp_python::stub_info()?.generate()?;
    let stub = Path::new(env!("CARGO_MANIFEST_DIR")).join("fpp_python.pyi");
    inject_union_aliases(&stub)?;
    Ok(())
}

/// Insert `<Alias>: typing.TypeAlias = <expansion>` lines just before the first
/// class/def definition (after the import block), register them in `__all__`, and
/// normalize union spacing.
fn inject_union_aliases(path: &Path) -> std::io::Result<()> {
    let text = fs::read_to_string(path)?;

    let mut aliases = String::new();
    let mut alias_names = Vec::new();
    for (name, rhs) in fpp_python::union_aliases() {
        aliases.push_str(&format!("{name}: typing.TypeAlias = {rhs}\n"));
        alias_names.push(name);
    }
    aliases.push('\n');

    // Insert at the start of the first top-level definition (definitions follow
    // the import block and `__all__`). Any decorators the generator emits on that
    // definition — `@typing.final` on every non-`subclass` pyclass — must stay
    // attached to it, so back up over them: landing between a decorator and its
    // class would apply the decorator to an alias assignment.
    let lines: Vec<&str> = text.lines().collect();
    let first_def = lines
        .iter()
        .position(|l| l.starts_with("class ") || l.starts_with("def "))
        .unwrap_or(lines.len());
    let mut at = first_def;
    while at > 0 && lines[at - 1].starts_with('@') {
        at -= 1;
    }
    // Byte offset of the start of line `at`.
    let insert_at = lines[..at]
        .iter()
        .map(|l| l.len() + 1)
        .sum::<usize>()
        .min(text.len());

    let mut out = String::with_capacity(text.len() + aliases.len());
    out.push_str(&text[..insert_at]);
    out.push_str(&aliases);
    out.push_str(&text[insert_at..]);

    let out = add_to_dunder_all(&out, &alias_names);
    fs::write(path, normalize_union_spacing(&out))
}

/// Merge `names` into the generated `__all__` list, keeping it sorted (as
/// pyo3-stub-gen emits it) so the result is stable for the codegen-drift check.
///
/// Without this the injected aliases are absent from `__all__`, so a type checker
/// does not treat them as re-exported by the package.
fn add_to_dunder_all(text: &str, names: &[&str]) -> String {
    let (Some(start), Some(end)) = (text.find("__all__ = [\n"), text.find("\n]\n")) else {
        return text.to_string();
    };
    let body = &text[start + "__all__ = [\n".len()..end];

    let mut entries: Vec<&str> = body
        .lines()
        .map(|l| l.trim().trim_end_matches(',').trim_matches('"'))
        .filter(|l| !l.is_empty())
        .collect();
    entries.extend(names.iter().copied());
    entries.sort_unstable();
    entries.dedup();

    let rebuilt: String = entries.iter().map(|e| format!("    \"{e}\",\n")).collect();
    format!(
        "{}__all__ = [\n{rebuilt}]\n{}",
        &text[..start],
        &text[end + "\n]\n".len()..]
    )
}

/// Collapse `A  |  B` to `A | B`.
///
/// pyo3-stub-gen 0.23's type-expression qualifier keeps the original whitespace
/// tokens around a union `|` *and* re-emits its own, so every qualified union it
/// rewrites comes out double-spaced. Purely cosmetic, but it would otherwise land
/// in the committed stub.
fn normalize_union_spacing(text: &str) -> String {
    text.replace("  |  ", " | ")
}
