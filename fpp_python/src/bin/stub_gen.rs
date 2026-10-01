//! Dumps the type stubs from the pyclasses annotated with
//! `#[gen_stub_pyclass]` / `#[gen_stub_pymethods]`, via pyo3-stub-gen's inventory.
//!
//! Build/run WITHOUT the `extension-module` feature (a standalone executable
//! must link libpython):
//!   cargo run -p fpp_python --no-default-features --features stubgen --bin stub_gen
//!
//! Two files, one per module — `python/fpp/__init__.pyi` for the semantic layer and
//! the entry points, `python/fpp/ast.pyi` for the AST wrappers — written beside the
//! checked-in `__init__.py`/`ast.py` that maturin ships as the `fpp` package. See
//! `fpp::stub_info`: pyo3-stub-gen's own `generate()` is not used, because it insists
//! on `<module>/__init__.pyi` for every module in a mixed layout and `fpp.ast` is a
//! plain module, not a package.
//!
//! A union's variant classes are submitted to the inventory under their DOTTED
//! Python name (`Type.Alias`), which is the name every annotation referring to them
//! uses but is not a legal class header. [`nest_union_variants`] moves each one into
//! its base class before generation, so they come out as real nested classes; it
//! also adds the `Variant` closed-union alias, which pyo3-stub-gen has no way to
//! derive.
//!
//! Four text passes follow generation. `normalize_union_spacing` collapses the doubled
//! spacing pyo3-stub-gen leaves around a union `|`; [`unenum_leaf_mirrors`] corrects a
//! claim it hardcodes — it renders every `#[pyclass]` enum as `class X(enum.Enum)`,
//! which the leaf-enum mirrors are not; [`add_self_import`] restores the one import it
//! drops that these stubs need; and [`strip_trailing_whitespace`] cleans up after the
//! nested-class indenter.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use pyo3_stub_gen::generate::{ClassDef, MemberDef, Module};
use pyo3_stub_gen::{StubInfo, TypeInfo};

fn main() -> pyo3_stub_gen::Result<()> {
    let mut info = fpp::stub_info()?;
    assert_eq!(
        fpp::AST_MODULE,
        format!("{}.ast", fpp::PACKAGE),
        "the two generated layers disagree about the package they are in"
    );
    assert_eq!(
        fpp::PACKAGE,
        info.default_module_name,
        "the package the generated layers annotate with is not the one this stub is \
         being written for"
    );
    resolve_module_names(&mut info);
    nest_union_variants(&mut info);
    for (module_name, module) in &info.modules {
        let text = module.format_with_config(info.config.use_type_statement);
        let text = normalize_union_spacing(&text);
        let text = unenum_leaf_mirrors(&text);
        let text = add_self_import(&text, module_name);
        fs::write(
            stub_path(&info, module_name),
            strip_trailing_whitespace(&text),
        )?;
    }
    Ok(())
}

/// Where a module's stub lands: `python/fpp/__init__.pyi` for the package itself,
/// `python/fpp/<sub>.pyi` for a submodule.
fn stub_path(info: &StubInfo, module_name: &str) -> PathBuf {
    let root = info.python_root.join(&info.default_module_name);
    match module_name.strip_prefix(&format!("{}.", info.default_module_name)) {
        Some(sub) => root.join(format!("{sub}.pyi")),
        None => root.join("__init__.pyi"),
    }
}

/// Re-resolve every class's `ModuleRef::Default` against the real module name.
///
/// pyo3-stub-gen does this once, when it first sees a `#[pyclass]` — which is before
/// it has attached anything from a `#[pymethods]` block, so every getter and method
/// signature still carries the placeholder. That is invisible in a one-module stub
/// (an unresolved reference and a same-module one render identically) and wrong the
/// moment there are two: `AstNode.definition` would come out as a bare `Symbol.Variant`
/// in `fpp/ast.pyi`, with nothing importing it. Resolving again is idempotent.
fn resolve_module_names(info: &mut StubInfo) {
    let default = info.default_module_name.clone();
    for module in info.modules.values_mut() {
        for class in module.class.values_mut() {
            class.resolve_default_modules(&default);
        }
    }
}

/// Move each union's variant classes into the base class they belong to, and give
/// the base its `Variant` closed-union alias.
///
/// Mutating [`StubInfo`] rather than the emitted text is what keeps this honest:
/// pyo3-stub-gen already renders `ClassDef::classes` recursively, so nesting is a
/// move between two public collections and never has to parse a class header. It
/// also drops the variants from `__all__` for free — `Module::collect_all_items`
/// only walks the top-level class map.
///
/// Nesting a variant binds its bare name in the base's class scope, which is what
/// makes two things matter here:
///
/// 1. The `Variant` right-hand side must stay qualified by its base (`Type.Alias | …`,
///    which the macro's `union_typeinfo` produces). A bare right-hand side would
///    resolve some members to the nested class and others to a same-named
///    module-level class.
/// 2. An annotation inside the class body naming a module-level class that one of the
///    variants shadows is ambiguous, and mypy and pyright resolve it DIFFERENTLY.
///    [`fix_shadowed_annotations`] qualifies those against the module itself
///    (`Ident` -> `fpp.ast.Ident`), which reaches past every class scope.
fn nest_union_variants(info: &mut StubInfo) {
    let default_module = info.default_module_name.clone();
    // Lift every dotted class out of the top level of every module, bucketed by the
    // `(module, base)` it names. The dot is the whole instruction: nothing else in
    // these modules carries one — leaf enums live in `Module::enum_`, and the
    // exception type is undotted.
    let mut buckets: BTreeMap<(String, String), Vec<ClassDef>> = BTreeMap::new();
    // Every module-level type name, per module, captured before anything moves, so a
    // shadowed annotation can be checked against what that module really exports.
    // Both maps: a leaf enum is an `enum_`, not a `class`, and `Binop`/`Unop` are
    // leaf enums shadowed by the `ExprKind` variants of the same name.
    let mut top_level: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (module_name, module) in info.modules.iter_mut() {
        top_level.insert(module_name.clone(), module_type_names(module));
        let dotted: Vec<_> = module
            .class
            .iter()
            .filter(|(_, c)| c.name.contains('.'))
            .map(|(id, _)| *id)
            .collect();
        for id in dotted {
            let class = module.class.remove(&id).expect("just collected this id");
            let base = class
                .name
                .rsplit_once('.')
                .expect("filtered on containing a dot")
                .0
                .to_string();
            buckets
                .entry((module_name.clone(), base))
                .or_default()
                .push(class);
        }
    }

    for stub in fpp::union_stubs() {
        let module_name = stub.module.unwrap_or(&default_module).to_string();
        let base_name = stub.base;
        let mut bucket = buckets
            .remove(&(module_name.clone(), base_name.to_string()))
            .unwrap_or_else(|| {
                panic!("union `{module_name}.{base_name}` submitted no dotted variant classes")
            });
        let module = info
            .modules
            .get_mut(&module_name)
            .unwrap_or_else(|| panic!("the `{module_name}` module is in the inventory"));
        let owners: Vec<&mut ClassDef> = module
            .class
            .values_mut()
            .filter(|c| c.name == base_name)
            .collect();
        let [owner] = <[&mut ClassDef; 1]>::try_from(owners).unwrap_or_else(|v| {
            panic!(
                "expected exactly one class named `{base_name}` in `{module_name}`, found {}",
                v.len()
            )
        });
        assert_eq!(
            bucket.len(),
            stub.variants.len(),
            "union `{base_name}` declares {} variants but submitted {} classes",
            stub.variants.len(),
            bucket.len()
        );

        // Declaration order, matching the order `union_typeinfo` built the RHS in.
        for (dotted_name, bare_name) in &stub.variants {
            let at = bucket
                .iter()
                .position(|c| c.name == *dotted_name)
                .unwrap_or_else(|| panic!("no class submitted for `{dotted_name}`"));
            let mut class = bucket.remove(at);
            class.name = bare_name;
            owner.classes.push(class);
        }

        let own_top_level = &top_level[&module_name];
        fix_shadowed_annotations(owner, &stub.variants, &module_name, own_top_level);
        owner.attrs.push(MemberDef {
            name: "Variant",
            r#type: TypeInfo::with_module("typing.TypeAlias", "typing".into()),
            doc: "",
            default: Some(stub.variant_rhs),
            deprecated: None,
        });
    }

    assert!(
        buckets.is_empty(),
        "dotted classes with no owning union: {:?}",
        buckets.keys().collect::<Vec<_>>()
    );
    let leftover: Vec<(&String, &str)> = info
        .modules
        .iter()
        .flat_map(|(m, module)| {
            module
                .class
                .values()
                .map(|c| c.name)
                .filter(|n| n.contains('.'))
                .map(move |n| (m, n))
        })
        .collect();
    assert!(
        leftover.is_empty(),
        "dotted classes left at top level: {leftover:?}"
    );
}

/// Every type name a module declares at its top level — classes and leaf-enum
/// mirrors alike.
fn module_type_names(module: &Module) -> BTreeSet<String> {
    module
        .class
        .values()
        .map(|c| c.name.to_string())
        .chain(module.enum_.values().map(|e| e.name.to_string()))
        .collect()
}

/// Rewrite every annotation inside `owner`'s class body — its own members AND its
/// nested variants' — that names a module-level class one of those variants shadows,
/// qualifying it against the module itself (`Ident` -> `fpp.ast.Ident`).
///
/// Nesting a variant binds its bare name in the base's class scope, where it captures
/// any bare reference to a same-named module-level class. The nested classes matter as
/// much as the base, because that scope covers them too: `ExprKind.Dot.id` returns the
/// `Ident` *node* while `ExprKind.Ident` is a sibling variant, and `ExprKind.Binop.op`
/// returns the `Binop` *leaf enum* from inside the class that is itself named `Binop`.
/// Left bare, mypy and pyright resolve these DIFFERENTLY — mypy takes the variant and
/// silently retypes a public getter, pyright takes the module-level class — so neither
/// reading can be relied on.
///
/// `fpp.ast.Ident` is the one absolute qualifier Python annotations have: a self-import
/// of the module the stub already is. It reaches past every class scope, needs no
/// invented alias name, and says plainly which `Ident` is meant. Six annotations need
/// it today; a seventh introduced upstream is handled without anyone noticing, which is
/// the point of qualifying rather than keeping a list.
///
/// A bare name in an annotation always denotes a module-level class — a reference to a
/// sibling variant is already written dotted — so [`qualify_shadowed`] asserts the name
/// really is one before rewriting.
fn fix_shadowed_annotations(
    owner: &mut ClassDef,
    variants: &[(&str, &str)],
    module: &str,
    top_level: &BTreeSet<String>,
) {
    let bare: Vec<&str> = variants.iter().map(|(_, b)| *b).collect();
    let base = owner.name;
    qualify_shadowed(own_annotations(owner), base, &bare, module, top_level);
    for nested in owner.classes.iter_mut() {
        qualify_shadowed(own_annotations(nested), base, &bare, module, top_level);
    }
}

/// Every type expression a class writes about its OWN members — getters, setters, and
/// its methods' parameters and returns. Does not descend into nested classes.
fn own_annotations(c: &mut ClassDef) -> Vec<&mut TypeInfo> {
    let mut out: Vec<&mut TypeInfo> = Vec::new();
    for (getter, setter) in c.getter_setters.values_mut() {
        out.extend(getter.iter_mut().map(|m| &mut m.r#type));
        out.extend(setter.iter_mut().map(|m| &mut m.r#type));
    }
    for overloads in c.methods.values_mut() {
        for m in overloads {
            out.push(&mut m.r#return);
            // `Parameters` exposes its four buckets but has no mutable iterator.
            let p = &mut m.parameters;
            out.extend(
                p.positional_only
                    .iter_mut()
                    .chain(p.positional_or_keyword.iter_mut())
                    .chain(p.keyword_only.iter_mut())
                    .chain(p.varargs.iter_mut())
                    .chain(p.varkw.iter_mut())
                    .map(|p| &mut p.type_info),
            );
        }
    }
    out
}

/// Qualify each bare name in `annotations` that one of `bare` would shadow.
///
/// Works on the RENDERED expression, not the stored one: what a class-scope binding
/// can capture is whatever ends up written in the body, and a cross-module reference
/// is stored qualified (`ast.Ident`) only to be de-qualified again in its own module.
/// Any annotation this touches is therefore frozen at its rendering for `module` —
/// which is the one module it can appear in, since a class belongs to exactly one.
fn qualify_shadowed(
    annotations: Vec<&mut TypeInfo>,
    base: &str,
    bare: &[&str],
    module: &str,
    top_level: &BTreeSet<String>,
) {
    for info in annotations {
        let rendered = info.qualified_for_module(module);
        let shadowed: Vec<&str> = bare
            .iter()
            .copied()
            .filter(|b| names_bare_ident(&rendered, b))
            .collect();
        if shadowed.is_empty() {
            continue;
        }
        let mut name = rendered;
        for shadow in shadowed {
            assert!(
                top_level.contains(shadow),
                "`{base}` annotates a member with the bare name `{shadow}` in \
                 `{name}`, but `{module}` declares no class of that name to qualify it \
                 against",
            );
            name = replace_bare_ident(&name, shadow, &format!("{module}.{shadow}"));
        }
        info.name = name;
        // Rendered once, here: nothing downstream may re-qualify it.
        info.source_module = None;
        info.type_refs.clear();
        // The self-import that makes the qualifier resolve. `Module`'s import pass
        // drops an import of the module it is writing, which is right everywhere
        // except here — `add_self_import` puts it back.
        info.import.insert(module.into());
    }
}

/// Whether `expr` uses `ident` as a bare (undotted) identifier. A dotted reference
/// is already unambiguous; only a bare name can be captured by a class-scope
/// binding.
fn names_bare_ident(expr: &str, ident: &str) -> bool {
    bare_idents(expr).any(|i| i == ident)
}

/// The bare identifiers of a type expression, skipping dotted references.
fn bare_idents(expr: &str) -> impl Iterator<Item = &str> {
    expr.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
        .filter(|i| !i.is_empty() && !i.contains('.'))
}

/// Replace every bare occurrence of `from` in a type expression with `to`, leaving
/// dotted references (`Type.Topology`) and longer identifiers alone.
fn replace_bare_ident(expr: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(expr.len());
    let mut rest = expr;
    while let Some(at) = rest.find(from) {
        let (before, tail) = rest.split_at(at);
        let after = &tail[from.len()..];
        let boundary_before = before
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '.');
        let boundary_after = after
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric() && c != '_');
        out.push_str(before);
        if boundary_before && boundary_after {
            out.push_str(to);
        } else {
            out.push_str(from);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Rewrite `class X(enum.Enum):` to a plain `class X:` whose members are declared
/// `typing.ClassVar[X]`.
fn unenum_leaf_mirrors(text: &str) -> String {
    const HEADER_SUFFIX: &str = "(enum.Enum):";
    let mut out = String::with_capacity(text.len());
    let mut class_name: Option<String> = None;
    for line in text.lines() {
        // A line at column 0 ends the previous class body; only an `enum.Enum`
        // header opens one.
        if !line.is_empty() && !line.starts_with(char::is_whitespace) {
            class_name = line
                .strip_prefix("class ")
                .and_then(|rest| rest.strip_suffix(HEADER_SUFFIX))
                .map(str::to_string);
            match &class_name {
                Some(name) => out.push_str(&format!("class {name}:\n")),
                None => {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            continue;
        }
        let member = class_name.as_deref().and_then(|name| {
            let member = line.strip_prefix("    ")?.strip_suffix(" = ...")?;
            let is_ident = !member.is_empty()
                && member.chars().all(|c| c.is_alphanumeric() || c == '_')
                && !member.starts_with(|c: char| c.is_numeric());
            is_ident.then(|| format!("    {member}: typing.ClassVar[{name}]\n"))
        });
        match member {
            Some(rewritten) => out.push_str(&rewritten),
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    drop_dead_enum_import(&out)
}

/// Drop `import enum` once nothing in the stub uses the module.
fn drop_dead_enum_import(text: &str) -> String {
    let used = text
        .lines()
        .any(|l| l.contains("(enum.") || l.contains(": enum.") || l.contains("-> enum."));
    if used {
        return text.to_string();
    }
    text.lines()
        .filter(|l| *l != "import enum")
        .map(|l| format!("{l}\n"))
        .collect()
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

/// Add the stub's own `import <module>` when an annotation qualifies against it.
///
/// [`qualify_shadowed`] writes `fpp.ast.Ident` to reach past a nested class that
/// shadows `Ident`, and records the matching import — but pyo3-stub-gen drops an
/// import of the module it is generating, which is right everywhere except here. A
/// `.pyi` is never executed, so the self-import cannot recurse; it exists purely to
/// bind the name the qualifier reads through.
fn add_self_import(text: &str, module: &str) -> String {
    let qualifier = format!("{module}.");
    let needed = text
        .lines()
        .any(|l| !l.starts_with("import ") && l.contains(&qualifier));
    let already = text.lines().any(|l| l == format!("import {module}"));
    if !needed || already {
        return text.to_string();
    }
    // After the last of the generated imports, so the block stays contiguous.
    let mut out = String::with_capacity(text.len() + module.len() + 8);
    let last_import = text
        .lines()
        .enumerate()
        .filter(|(_, l)| l.starts_with("import ") || l.starts_with("from "))
        .map(|(i, _)| i)
        .last();
    for (i, line) in text.lines().enumerate() {
        out.push_str(line);
        out.push('\n');
        if Some(i) == last_import {
            out.push_str(&format!("import {module}\n"));
        }
    }
    out
}

/// Drop trailing whitespace from every line.
///
/// pyo3-stub-gen indents a nested class block line by line, its trailing blank line
/// included, so each nested class leaves behind a line of four spaces. The committed
/// stub should not carry those, and trimming is idempotent so it cannot churn the
/// codegen-drift check.
fn strip_trailing_whitespace(text: &str) -> String {
    text.lines()
        .map(|l| format!("{}\n", l.trim_end()))
        .collect()
}
