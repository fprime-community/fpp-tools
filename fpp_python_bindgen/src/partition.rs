//! The shared `fpp_ast` grammar partition.
//!
//! Parses the `fpp_ast` source (a pinned fpp-tools checkout) with `syn` and
//! classifies every grammar type by name into nodes / unions / kind-enums /
//! leaves. This partition is consulted TWICE: [`ast_emit`](super::ast_emit)
//! renders it into `fpp_python/src/ast/defs.rs`, and the semantic generator
//! ([`sem`](super::sem)) resolves each `fpp_ast::X` reference against it (via
//! [`AstClass::classify_ast_ref`]) instead of guessing from a name prefix.
//!
//! Classification reuses the annotations the fpp-tools proc-macros already
//! consume: `#[ast]` on a struct = a node; `#[ast]` on an enum = a transparent
//! union; a non-`#[ast]` enum deriving a `*Walkable` = a "kind enum"; any other
//! enum = a "leaf enum" (rendered to a string); `#[visitable(ignore)]` = scalar
//! leaf. `Name` collapses to a Python `str`; `Ident` stays a node.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use syn::{Attribute, Fields, GenericArgument, Item, PathArguments, Type};

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Card {
    One,
    Opt,
    Vec,
    OptVec,
}

#[derive(Clone)]
pub enum Shape {
    Str,
    Bool,
    Leaf(String), // leaf enum -> String (rendered)
    LeafOpt(String),
    // Collapsed string leaf (e.g. `Name` / `LitString`): the value is a named
    // scalar sub-field cloned to a `str`. The `String` is that accessor field,
    // discovered from the source struct and carried into the DSL as `str(<acc>)`.
    StrLeaf(String),
    StrLeafOpt(String),
    Skip,                // scalar we don't model (e.g. fpp_core::Span)
    Child(Card, String), // node type (struct or union), or Ident
    Kind(String),        // inline kind-enum field
}

pub struct FieldDef {
    pub name: String,
    pub shape: Shape,
}

pub struct StructDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

pub struct UnionDef {
    pub name: String,
    pub variants: Vec<(String, String)>, // (variant ident, inner node type)
}

pub enum KindField {
    Unnamed(Shape),       // single-field tuple variant payload
    Named(Vec<FieldDef>), // struct variant
    Unit,
}

pub struct KindVariant {
    pub name: String,
    pub field: KindField,
}

pub struct KindDef {
    pub name: String,
    pub variants: Vec<KindVariant>,
}

/// The translation-unit root: a non-`#[ast]` walkable container whose sole field
/// is a `Vec` of a member union. Emitted as the DSL's `root` directive so the
/// walk's entry point is data-driven rather than hard-coded in the macro.
pub struct RootDef {
    pub container: String,
    /// Field access reaching the member `Vec`: a tuple index (`0`) or field name.
    pub field: String,
    pub member: String,
}

/// Whether a leaf-enum variant carries a payload (dropped in the Python mirror).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Payload {
    Unit,
    Tuple,
    Struct,
}

#[derive(Default)]
pub struct Registry {
    pub node_structs: BTreeMap<String, StructDef>,
    pub unions: BTreeMap<String, UnionDef>,
    pub kinds: BTreeMap<String, KindDef>,
    pub leaf_enums: BTreeSet<String>,
    // leaf enum -> its variants (name + payload kind), for the Python-enum mirror
    pub leaf_defs: BTreeMap<String, Vec<(String, Payload)>>,
    pub aliases: BTreeMap<String, Type>,
    // categorization sets (name membership)
    pub is_node: BTreeSet<String>,
    pub is_union: BTreeSet<String>,
    pub is_kind: BTreeSet<String>,
    // leaf enums actually referenced (need a Python-enum mirror). Seeded from the
    // `fpp_ast` fields/variants that reference them, then cross-fed by the sem
    // generator via `register_used_leaf` (so a leaf reachable only through the
    // semantic layer still gets a Python-enum mirror in `ast/defs.rs`).
    pub used_leaves: BTreeSet<String>,
    // collapsed string-leaf type name -> its scalar accessor field (e.g.
    // `Name`/`LitString` -> `data`), discovered from the source struct.
    pub str_leaf_accessor: BTreeMap<String, String>,
    // the translation-unit root container (walk entry point), if detected.
    pub root: Option<RootDef>,
    // Node wrappers whose Python name collides with a semantic class of the same
    // name (built during navigation, but not stubbed, and opaque as children).
    // Derived by the driver from the RESOLVED semantic Python names ∩ node names
    // and installed via `set_shadowed`; rendered into the DSL's `shadowed {…}`.
    pub shadowed: BTreeSet<String>,
}

const EXCLUDE_NODE: &[&str] = &["Name"]; // collapsed to str; never a node wrapper

/// Types whose value collapses to a Python `str` via a single scalar sub-field
/// (`Name.data`, `LitString.data`) rather than becoming a walked child. Emitted
/// into the DSL as the `str(<field>)` leaf form; asserted never to appear as a
/// walked child (`assert_string_leaves`).
const STRING_LEAF: &[&str] = &["Name", "LitString"];

/// `fpp_ast/src` files with no `#[ast]` items (traits + visitor scaffolding);
/// skipped by the source walk. Everything else under `src/` is parsed, so a new
/// grammar file is picked up automatically rather than silently ignored.
///
/// Keyed by filename: renaming one of these (or adding another non-grammar helper
/// that declares a plain enum) could let a non-grammar enum be classified as a
/// leaf. Harmless unless that enum is also referenced (only referenced leaves are
/// emitted), and `assert_classification` guards the inverse (a grammar type going
/// missing); revisit as an attribute-driven allowlist if scaffolding files grow.
const SRC_DENYLIST: &[&str] = &["node.rs", "visit.rs"];

// ---------------------------------------------------------------------------
// AST-reference classification (consulted by the sem generator)
// ---------------------------------------------------------------------------

/// The result of resolving a bare `fpp_ast::<name>` reference against the real
/// partition (Rule R1). Shadowing is NOT applied here — a shadowed AST node is
/// still reported as [`AstRef::AstDef`] and demoted to a skip later, by the
/// driver's normalize phase, once the resolved semantic names are known.
pub enum AstRef {
    /// A recorded walk node → bridged through `crate::ast::<Name>` (`astdef`).
    AstDef(String),
    /// A fieldless `fpp_ast` enum → `leaf(crate::ast::<Name>)`.
    Leaf(String),
    /// No usable mirror (payload-bearing union, kind enum, collapse type, or
    /// unknown) — the referencing field/variant is emitted as `skip`.
    Skip(String),
}

/// The membership sets the sem generator needs to classify an `fpp_ast::X`
/// reference — an owned, immutable snapshot handed to the reflection context so
/// the mutable [`Registry`] (used-leaf cross-feed + shadowed set) stays free.
#[derive(Clone, Default)]
pub struct AstClass {
    pub is_node: BTreeSet<String>,
    pub is_union: BTreeSet<String>,
    pub is_kind: BTreeSet<String>,
    pub leaf_enums: BTreeSet<String>,
}

impl AstClass {
    /// Rule R1: resolve a bare `fpp_ast::<name>` reference against the partition.
    pub fn classify_ast_ref(&self, name: &str) -> AstRef {
        if self.is_node.contains(name) {
            AstRef::AstDef(name.to_string())
        } else if self.is_union.contains(name) {
            AstRef::Skip(format!(
                "fpp_ast::{name} (payload-bearing AST union; no leaf mirror in crate::ast)"
            ))
        } else if self.is_kind.contains(name) {
            AstRef::Skip(format!("fpp_ast::{name} (AST kind enum; no leaf mirror)"))
        } else if self.leaf_enums.contains(name) {
            AstRef::Leaf(name.to_string())
        } else {
            // Covers `Name` (collapsed to str; excluded from the node set) and any
            // grammar type the semantic layer references but the AST bindings do
            // not surface as a wrapper.
            AstRef::Skip(format!("fpp_ast::{name} (no crate::ast wrapper)"))
        }
    }
}

impl Registry {
    /// The immutable classification snapshot for the sem generator.
    pub fn ast_class(&self) -> AstClass {
        AstClass {
            is_node: self.is_node.clone(),
            is_union: self.is_union.clone(),
            is_kind: self.is_kind.clone(),
            leaf_enums: self.leaf_enums.clone(),
        }
    }

    /// Every parsed AST-node wrapper name (the candidates a semantic class name
    /// can shadow).
    pub fn node_names(&self) -> &BTreeSet<String> {
        &self.is_node
    }

    /// Record a leaf enum the semantic layer references, so `ast/defs.rs` emits a
    /// Python-enum mirror for it even if no `fpp_ast` field/variant uses it.
    pub fn register_used_leaf(&mut self, name: String) {
        self.used_leaves.insert(name);
    }

    /// Install the derived entity-shadowed node set (rendered into `shadowed {…}`).
    pub fn set_shadowed(&mut self, shadowed: BTreeSet<String>) {
        self.shadowed = shadowed;
    }
}

// ---------------------------------------------------------------------------
// Build
// ---------------------------------------------------------------------------

/// Parse the `fpp_ast` sources under `src_dir` and build the classified
/// partition, running the classification/string-leaf tripwires.
pub fn build(src_dir: &Path) -> Registry {
    let mut items: Vec<Item> = Vec::new();
    for path in fpp_ast_sources(src_dir) {
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let file =
            syn::parse_file(&src).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
        items.extend(file.items);
    }

    let reg = build_registry(&items);
    assert!(
        reg.root.is_some(),
        "no translation-unit root container detected (a non-#[ast] walkable struct \
         with a single `Vec<Union>` field)"
    );
    assert_string_leaves(&reg);
    assert_classification(&reg);
    reg
}

/// The `fpp_ast` grammar source files to parse: every `*.rs` under `src_dir`
/// except the [`SRC_DENYLIST`], sorted for deterministic (idempotent) output.
fn fpp_ast_sources(src_dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(src_dir)
        .unwrap_or_else(|e| panic!("read dir {}: {e}", src_dir.display()))
        .map(|e| e.expect("read dir entry").path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("rs"))
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            !SRC_DENYLIST.contains(&name)
        })
        .collect();
    paths.sort();
    paths
}

fn build_registry(items: &[Item]) -> Registry {
    let mut reg = Registry::default();

    // Pass 1: categorize every type by name.
    for item in items {
        match item {
            Item::Struct(s) if has_ast(&s.attrs) => {
                let name = s.ident.to_string();
                if STRING_LEAF.contains(&name.as_str()) {
                    reg.str_leaf_accessor
                        .insert(name.clone(), string_leaf_accessor(&name, &s.fields));
                }
                if !EXCLUDE_NODE.contains(&name.as_str()) {
                    reg.is_node.insert(name);
                }
            }
            Item::Enum(e) if has_ast(&e.attrs) => {
                reg.is_union.insert(e.ident.to_string());
            }
            Item::Enum(e) if has_derive_walkable(&e.attrs) => {
                reg.is_kind.insert(e.ident.to_string());
            }
            Item::Enum(e) => {
                let name = e.ident.to_string();
                reg.leaf_enums.insert(name.clone());
                let variants = e
                    .variants
                    .iter()
                    .map(|v| {
                        let payload = match &v.fields {
                            Fields::Unit => Payload::Unit,
                            Fields::Unnamed(_) => Payload::Tuple,
                            Fields::Named(_) => Payload::Struct,
                        };
                        (v.ident.to_string(), payload)
                    })
                    .collect();
                reg.leaf_defs.insert(name, variants);
            }
            Item::Type(t) => {
                reg.aliases.insert(t.ident.to_string(), (*t.ty).clone());
            }
            _ => {}
        }
    }
    // `Ident` is an #[ast] node; make sure it's treated as a node target.
    reg.is_node.insert("Ident".to_string());

    // The walk entry point: the top-level walkable container (e.g. `TransUnit`).
    reg.root = detect_root(items, &reg.is_union);

    // Pass 2: build defs, classifying fields against the category sets.
    for item in items {
        match item {
            Item::Struct(s) if has_ast(&s.attrs) => {
                let name = s.ident.to_string();
                if EXCLUDE_NODE.contains(&name.as_str()) {
                    continue;
                }
                let (fields, used) = classify_fields(&s.fields, &reg);
                reg.node_structs
                    .insert(name.clone(), StructDef { name, fields });
                for l in used {
                    reg.used_leaves.insert(l);
                }
            }
            Item::Enum(e) if has_ast(&e.attrs) => {
                let mut variants = Vec::new();
                for v in &e.variants {
                    if let Fields::Unnamed(f) = &v.fields
                        && let Some(first) = f.unnamed.first()
                        && let Some(inner) = type_last_ident(&first.ty)
                    {
                        variants.push((v.ident.to_string(), inner));
                    }
                }
                reg.unions.insert(
                    e.ident.to_string(),
                    UnionDef {
                        name: e.ident.to_string(),
                        variants,
                    },
                );
            }
            Item::Enum(e) if has_derive_walkable(&e.attrs) => {
                let mut variants = Vec::new();
                let mut used = Vec::new();
                for v in &e.variants {
                    // A `#[visitable(ignore)]` variant is a scalar leaf: its
                    // payload (String / bool / a leaf enum) is not a child node.
                    let vign = has_visitable_ignore(&v.attrs);
                    let field = match &v.fields {
                        Fields::Unit => KindField::Unit,
                        Fields::Unnamed(f) => {
                            let first = f.unnamed.first().unwrap();
                            let sh = classify_one(&first.attrs, &first.ty, &reg, &mut used, vign);
                            KindField::Unnamed(sh)
                        }
                        Fields::Named(f) => {
                            let mut fields = Vec::new();
                            for fd in &f.named {
                                let sh = classify_one(&fd.attrs, &fd.ty, &reg, &mut used, vign);
                                fields.push(FieldDef {
                                    name: fd.ident.as_ref().unwrap().to_string(),
                                    shape: sh,
                                });
                            }
                            KindField::Named(fields)
                        }
                    };
                    variants.push(KindVariant {
                        name: v.ident.to_string(),
                        field,
                    });
                }
                reg.kinds.insert(
                    e.ident.to_string(),
                    KindDef {
                        name: e.ident.to_string(),
                        variants,
                    },
                );
                for l in used {
                    reg.used_leaves.insert(l);
                }
            }
            _ => {}
        }
    }

    reg
}

/// Find the walk entry point: a non-`#[ast]` walkable struct whose single field
/// is a `Vec<Union>`. Structural (not name-keyed), so a rename of the container
/// is picked up automatically. Panics if more than one candidate exists.
fn detect_root(items: &[Item], is_union: &BTreeSet<String>) -> Option<RootDef> {
    let mut found: Option<RootDef> = None;
    for item in items {
        let Item::Struct(s) = item else { continue };
        if has_ast(&s.attrs) || !has_derive_walkable(&s.attrs) {
            continue;
        }
        let (field, ty) = match &s.fields {
            Fields::Unnamed(f) if f.unnamed.len() == 1 => {
                ("0".to_string(), &f.unnamed.first().unwrap().ty)
            }
            Fields::Named(f) if f.named.len() == 1 => {
                let fd = f.named.first().unwrap();
                (fd.ident.as_ref().unwrap().to_string(), &fd.ty)
            }
            _ => continue,
        };
        let (card, inner) = container(ty);
        if card != Card::Vec {
            continue;
        }
        let Some(member) = type_last_ident(&inner) else {
            continue;
        };
        if !is_union.contains(&member) {
            continue;
        }
        let rd = RootDef {
            container: s.ident.to_string(),
            field,
            member,
        };
        if let Some(prev) = &found {
            panic!(
                "multiple root containers detected (`{}` and `{}`); the walk entry \
                 point is ambiguous",
                prev.container, rd.container
            );
        }
        found = Some(rd);
    }
    found
}

/// Every field/variant `Shape` in the registry, for whole-grammar assertions.
fn all_shapes(reg: &Registry) -> impl Iterator<Item = &Shape> {
    let node_fields = reg
        .node_structs
        .values()
        .flat_map(|d| &d.fields)
        .map(|f| &f.shape);
    let kind_fields = reg
        .kinds
        .values()
        .flat_map(|d| &d.variants)
        .flat_map(|v| match &v.field {
            KindField::Unnamed(sh) => std::slice::from_ref(sh).iter().collect::<Vec<_>>(),
            KindField::Named(fs) => fs.iter().map(|f| &f.shape).collect(),
            KindField::Unit => Vec::new(),
        });
    node_fields.chain(kind_fields)
}

/// Guard the string-leaf / walked-child ambiguity: a collapse type
/// ([`STRING_LEAF`]) that ever appears as a walked child would silently vanish
/// from the model, since its DSL `str(..)` leaf token differs from a node
/// reference. In the current grammar these types are only ever leaves; abort if
/// `fpp_ast` starts using one as a real child so the coupling stays honest.
fn assert_string_leaves(reg: &Registry) {
    for shape in all_shapes(reg) {
        if let Shape::Child(_, ty) = shape {
            assert!(
                !STRING_LEAF.contains(&ty.as_str()),
                "collapse type `{ty}` is used as a walked child; it would be dropped \
                 from the model — teach the generator to emit it as a node reference"
            );
        }
    }
}

/// Guard against a silent classification shift if an `fpp_macros` attr/derive is
/// renamed: kind-enum / node / union / leaf detection keys on the literal
/// `#[ast]` / `VisitorWalkable` / `DirectWalkable` names, so a rename would move
/// whole categories elsewhere. Assert a few stable anchors (one per category)
/// are classified as expected, and that no category came back empty.
fn assert_classification(reg: &Registry) {
    fn present<V>(set: &BTreeMap<String, V>, name: &str, cat: &str) {
        assert!(
            set.contains_key(name),
            "`{name}` is no longer classified as a {cat} — an `fpp_macros` \
             attr/derive rename may have silently reclassified the grammar"
        );
    }
    present(&reg.node_structs, "DefComponent", "node");
    present(&reg.node_structs, "DefModule", "node");
    present(&reg.unions, "ModuleMember", "union");
    present(&reg.kinds, "ExprKind", "kind");
    present(&reg.kinds, "TypeNameKind", "kind");
    assert!(
        reg.leaf_enums.contains("ComponentKind"),
        "`ComponentKind` is no longer classified as a leaf enum — an `fpp_macros` \
         attr/derive rename may have silently reclassified the grammar"
    );
    assert!(!reg.node_structs.is_empty(), "no nodes classified");
    assert!(!reg.unions.is_empty(), "no unions classified");
    assert!(!reg.kinds.is_empty(), "no kind enums classified");
    assert!(!reg.used_leaves.is_empty(), "no leaf enums referenced");
}

fn classify_fields(fields: &Fields, reg: &Registry) -> (Vec<FieldDef>, Vec<String>) {
    let mut out = Vec::new();
    let mut used = Vec::new();
    if let Fields::Named(named) = fields {
        for f in &named.named {
            let name = f.ident.as_ref().unwrap().to_string();
            if name == "node_id" {
                continue;
            }
            let shape = classify_one(&f.attrs, &f.ty, reg, &mut used, false);
            out.push(FieldDef { name, shape });
        }
    }
    (out, used)
}

fn classify_one(
    attrs: &[Attribute],
    ty: &Type,
    reg: &Registry,
    used: &mut Vec<String>,
    force_ignore: bool,
) -> Shape {
    let (card, inner) = container(ty);
    let name = type_last_ident(&inner).unwrap_or_default();
    if force_ignore || has_visitable_ignore(attrs) {
        return match name.as_str() {
            "String" => Shape::Str,
            "bool" => Shape::Bool,
            // An ignored collapse type (`LitString`) reads its scalar sub-field.
            n if reg.str_leaf_accessor.contains_key(n) => {
                let acc = reg.str_leaf_accessor[n].clone();
                if card == Card::Opt {
                    Shape::StrLeafOpt(acc)
                } else {
                    Shape::StrLeaf(acc)
                }
            }
            "Span" => Shape::Skip,
            n if reg.leaf_enums.contains(n) => {
                used.push(n.to_string());
                if card == Card::Opt {
                    Shape::LeafOpt(n.to_string())
                } else {
                    Shape::Leaf(n.to_string())
                }
            }
            _ => Shape::Skip,
        };
    }
    // Non-ignored -> child / name / kind. `Name` is a node-less collapse type,
    // so it reads its scalar sub-field even here; a non-ignored `LitString`
    // falls through to a walked child (caught by `assert_string_leaves`).
    match name.as_str() {
        "Name" => Shape::StrLeaf(reg.str_leaf_accessor["Name"].clone()),
        n if reg.is_kind.contains(n) => Shape::Kind(n.to_string()),
        n if reg.aliases.contains_key(n) && card == Card::One => {
            // Resolve the alias (e.g. FormalParamList = Vec<FormalParam>).
            let (c2, inner2) = container(&reg.aliases[n]);
            let n2 = type_last_ident(&inner2).unwrap_or_default();
            Shape::Child(c2, n2)
        }
        n => Shape::Child(card, n.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Committed-declaration readers (for the `--only` partial runs)
// ---------------------------------------------------------------------------

/// Parse the `shadowed { A, B, … }` line of an already-committed `ast/defs.rs`.
/// Used by `--only ast`, which regenerates the AST declaration without the sem
/// reflection that derives the shadowed set. `None` only when the file is
/// unreadable; a readable file with no (or an empty) `shadowed` line yields an
/// empty set — the full run emits `shadowed { … }` unconditionally, so a missing
/// line means a genuinely empty shadow set (or a pre-feature file), never a
/// dead-end.
pub fn read_committed_shadowed(path: &Path) -> Option<BTreeSet<String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let Some(line) = text
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("shadowed"))
    else {
        return Some(BTreeSet::new());
    };
    let inner = match line.split_once('{').and_then(|(_, r)| r.split_once('}')) {
        Some((inner, _)) => inner,
        None => return Some(BTreeSet::new()),
    };
    Some(
        inner
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

/// Parse the leaf-enum names declared in an already-committed `ast/defs.rs`
/// `leaves { … }` block. Used by `--only sem` to verify every `leaf(crate::ast::X)`
/// the semantic file references has a Python-enum mirror in the committed AST file.
pub fn read_committed_leaves(path: &Path) -> Option<BTreeSet<String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut out = BTreeSet::new();
    let mut in_block = false;
    for line in text.lines() {
        let l = line.trim();
        if !in_block {
            if l.starts_with("leaves") && l.ends_with('{') {
                in_block = true;
            }
            continue;
        }
        if l == "}" {
            break;
        }
        // Each entry is `<Name> { <variants> },`.
        if let Some(name) = l.split_whitespace().next() {
            out.insert(name.to_string());
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

// ---------------------------------------------------------------------------
// syn helpers
// ---------------------------------------------------------------------------

fn has_ast(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("ast"))
}

fn has_derive_walkable(attrs: &[Attribute]) -> bool {
    let mut found = false;
    for a in attrs {
        if !a.path().is_ident("derive") {
            continue;
        }
        let _ = a.parse_nested_meta(|m| {
            if let Some(id) = m.path.get_ident() {
                let s = id.to_string();
                if s == "VisitorWalkable" || s == "DirectWalkable" {
                    found = true;
                }
            }
            Ok(())
        });
    }
    found
}

fn has_visitable_ignore(attrs: &[Attribute]) -> bool {
    let mut found = false;
    for a in attrs {
        if !a.path().is_ident("visitable") {
            continue;
        }
        let _ = a.parse_nested_meta(|m| {
            if m.path.is_ident("ignore") {
                found = true;
            }
            Ok(())
        });
    }
    found
}

/// The scalar accessor field of a collapse type (`Name` / `LitString`): its
/// sole `String`-typed field, whose value the wrapper clones. Panics loudly if
/// the shape is not the expected single-`String`-field struct.
fn string_leaf_accessor(name: &str, fields: &Fields) -> String {
    let Fields::Named(named) = fields else {
        panic!("string-leaf type `{name}` must be a struct with named fields");
    };
    let mut found: Option<String> = None;
    for f in &named.named {
        if type_last_ident(&f.ty).as_deref() == Some("String") {
            let fname = f.ident.as_ref().unwrap().to_string();
            assert!(
                found.is_none(),
                "string-leaf type `{name}` has more than one `String` field"
            );
            found = Some(fname);
        }
    }
    found.unwrap_or_else(|| panic!("string-leaf type `{name}` has no `String` field"))
}

fn type_last_ident(ty: &Type) -> Option<String> {
    if let Type::Path(p) = ty {
        return p.path.segments.last().map(|s| s.ident.to_string());
    }
    None
}

fn inner_generic(ty: &Type) -> Option<Type> {
    if let Type::Path(p) = ty
        && let Some(seg) = p.path.segments.last()
        && let PathArguments::AngleBracketed(a) = &seg.arguments
    {
        for arg in &a.args {
            if let GenericArgument::Type(t) = arg {
                return Some(t.clone());
            }
        }
    }
    None
}

fn peel_box(ty: &Type) -> Type {
    if let Some(name) = type_last_ident(ty)
        && name == "Box"
        && let Some(inner) = inner_generic(ty)
    {
        return peel_box(&inner);
    }
    ty.clone()
}

/// Determine cardinality + the innermost payload type (Box is transparent).
fn container(ty: &Type) -> (Card, Type) {
    let ty = peel_box(ty);
    let name = type_last_ident(&ty).unwrap_or_default();
    if name == "Option"
        && let Some(inner) = inner_generic(&ty)
    {
        let inner = peel_box(&inner);
        if type_last_ident(&inner).as_deref() == Some("Vec")
            && let Some(elt) = inner_generic(&inner)
        {
            return (Card::OptVec, peel_box(&elt));
        }
        return (Card::Opt, inner);
    }
    if name == "Vec"
        && let Some(inner) = inner_generic(&ty)
    {
        return (Card::Vec, peel_box(&inner));
    }
    (Card::One, ty)
}
