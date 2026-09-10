//! The semantic declaration emitter — the semantic-layer analog of the
//! [`ast_emit`](super::ast_emit) AST pretty-printer.
//!
//! Reflects `fpp_analysis` from **rustdoc JSON** (compiler-resolved,
//! post-macro-expansion) and emits the checked-in `fpp_python/src/sem/defs.rs` —
//! a `fpp_python_macros::fpp_sem_bindings! { … }` invocation: a mechanical 1:1
//! mirror of the semantic data structures rooted at `fpp_analysis::Analysis`.
//! The `fpp_sem_bindings!` proc macro expands that declaration into the
//! read-only PyO3 wrappers.
//!
//! There are **no hand tables**: the set of unions / entities / leaf enums /
//! payloads is the transitive closure of reachable types from `Analysis`'s public
//! fields + eligible `&self`/`&Arc<Self>` methods. Type origin (`fpp_ast` vs
//! `fpp_core` vs `fpp_analysis` vs `std`) is a deterministic table lookup in the
//! JSON's `paths`/`external_crates`, never a name/prefix guess. An `fpp_ast`
//! reference is resolved against the shared [`partition`](super::partition) — the
//! REAL grammar classification — rather than a `Def*/Spec*` name prefix. A type
//! we cannot convert is emitted as `skip` (and logged), never a hard error.
//!
//! The JSON is either read directly (`--rustdoc-json`) or produced by invoking
//! nightly rustdoc via the `rustdoc-json` crate; the driver ([`super::main`])
//! parses it and asserts its `format_version` before handing the [`Crate`] here.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use rustdoc_types::{
    Crate, Enum, GenericArg, GenericArgs, GenericParamDefKind, Id, Item, ItemEnum, ItemKind,
    ItemSummary, Struct, StructKind, Type, Variant, VariantKind, Visibility,
};

use super::partition::{AstClass, AstRef};

// ---------------------------------------------------------------------------
// Shape vocabulary (mirrors fpp_python_macros::sem_bindings::Shape)
// ---------------------------------------------------------------------------

/// A field / method conversion shape. Local type references are carried by
/// rustdoc `Id` so the emitted Python name is resolved *after* the closure (once
/// name-disambiguation is known).
#[derive(Clone)]
enum Shape {
    Bool,
    I128,
    F64,
    Usize,
    Str,
    Node,
    Span,
    /// The unit type `()` → Python `None`. Only reachable as the `Ok` half of a
    /// `Result<(), E>` return (a pure check that raises on failure).
    Unit,
    /// A payload-bearing local enum → the union wrapper (by `Id`).
    Union(Id),
    /// A bare-payload-struct wrapped back into its owning union variant. Carries
    /// the union `Id` and the **native** variant ident.
    Rewrap(Id, String),
    /// A reflected local struct / alias → its own entity wrapper (by `Id`).
    Entity(Id),
    /// An all-unit local enum → `leaf(crate::sem::<PyName>)` (by `Id`).
    LeafEnum(Id),
    /// An `fpp_ast` fieldless enum → `leaf(crate::ast::<Name>)`.
    LeafAst(String),
    /// An `fpp_ast` `DefX`/`SpecX` node bridged through `crate::ast::<Name>`.
    AstDef(String),
    Opt(Box<Shape>),
    List(Box<Shape>),
    Map(Box<Shape>, Box<Shape>),
    Tuple(Vec<Shape>),
    Skip(String),
}

impl Shape {
    fn is_skip(&self) -> bool {
        matches!(self, Shape::Skip(_))
    }
}

// ---------------------------------------------------------------------------
// Reflected method
// ---------------------------------------------------------------------------

/// How a method parameter is supplied to the native call — the reflected mirror of
/// the macro's argkind vocabulary. Local type references are carried by rustdoc
/// `Id` (exactly as [`Shape`] does) so the emitted Python name is resolved *after*
/// the closure, once name-disambiguation is known.
#[derive(Clone)]
enum Arg {
    /// The injected `&Analysis` — a context place-expr, NOT a Python parameter.
    Analysis,
    /// A scalar argkind token: `i128` / `bool` / `usize` / `str` (a borrowed `&str`)
    /// / `string` (an owned `String`). Each token already encodes its own pass
    /// form, so [`ArgSpec::borrowed`] is not consulted for these.
    Scalar(&'static str),
    /// `fpp_core::Node` — the AST node handle, supplied by any `crate::ast::AstNode`
    /// wrapper (which stores exactly this handle).
    Node,
    /// `fpp_core::Span` — supplied by a `crate::ir_core::Span` wrapper.
    Span,
    /// `fpp_ast::<Name>` — supplied by the `crate::ast::<Name>` node wrapper, which
    /// reborrows the live AST node through its own `ModelData`.
    AstNode(String),
    /// A reflected local union (by `Id`) — supplied by its Python base class.
    Union(Id),
    /// A reflected local entity (by `Id`) — supplied by its Python class.
    Entity(Id),
    /// `Arc<inner>`.
    Arc(Box<Arg>),
    /// `Option<inner>`.
    Opt(Box<Arg>),
    /// `Vec<inner>` or `&[inner]`.
    List(Box<Arg>),
}

/// A reflected parameter: what supplies it, and whether the native signature takes
/// it by shared reference (`&T`) or by value (`T`).
#[derive(Clone)]
struct ArgSpec {
    arg: Arg,
    borrowed: bool,
}

#[derive(Clone)]
struct MethodDef {
    name: String,
    /// Associated fn whose first arg is the `&Arc<Self>` / `&Self` receiver.
    assoc: bool,
    /// (param name, spec) in source order (the receiver is excluded).
    params: Vec<(String, ArgSpec)>,
    ret: Shape,
    /// The native returns `&T`, not an owned `T` — `ret` describes `T` either way
    /// ([`classify`] peels the reference), so the DSL carries the distinction as a
    /// `ref` marker on the return. The macro converts *from* a `&native`: without
    /// this the call's `&T` would be referenced again, and a conversion that goes
    /// through a trait impl (`leaf`) or a dereference (`bool`, `span`) does not see
    /// through the extra `&`.
    ret_ref: bool,
    /// The native returns `Result<ret, E>`: the generated method raises instead of
    /// yielding the error. Carries `E`'s last path segment for the doc comment.
    throws: Option<String>,
    /// The `fpp_analysis` trait this method comes from, when it is not an inherent
    /// method. The generated code must bring that trait into scope for the call to
    /// resolve, so the path is carried into the DSL's `traits { … }` section.
    trait_path: Option<String>,
}

// ---------------------------------------------------------------------------
// Emission model
// ---------------------------------------------------------------------------

/// The `payloadkind` of a union variant (post-classification).
enum VariantPayload {
    Unit,
    /// A single-value variant → one `value`/`definition` getter of this shape.
    Value(Shape),
    /// A single-field tuple-struct payload → one `value` getter over `x.0`.
    Newtype(Shape),
    /// A bare multi-field payload struct → a `payload` decl (its `Id`).
    Struct(Id),
    /// An inline struct variant → one getter per named pub field.
    StructVariant(Vec<(String, Shape)>),
    /// A multi-field tuple variant (unsupported) → discarded (logged).
    Other,
}

struct VariantDef {
    native: String,
    payload: VariantPayload,
}

/// A reflected union (payload-bearing local enum).
pub struct UnionDef {
    id: Id,
    variants: Vec<VariantDef>,
    methods: Vec<MethodDef>,
}

/// A reflected entity / opaque (local struct, or an alias whose target is a local
/// struct). `impl_id` is the type whose impls supply the members (the alias
/// target, or the struct itself).
pub struct EntityDef {
    id: Id,
    impl_id: Id,
    fields: Vec<(String, Shape)>,
    methods: Vec<MethodDef>,
    /// `identity qualified_name` iff the type has a no-arg
    /// `qualified_name(&self)->String`.
    identity_qualified: bool,
}

/// A reflected `payload` struct (a union variant's bare payload).
pub struct PayloadDef {
    id: Id,
    fields: Vec<(String, Shape)>,
    methods: Vec<MethodDef>,
}

/// A reflected all-unit local enum (`leaf_enum`).
pub struct LeafEnumDef {
    id: Id,
    variants: Vec<(String, &'static str)>,
}

// ---------------------------------------------------------------------------
// Reflection context
// ---------------------------------------------------------------------------

pub struct Ctx<'a> {
    krate: &'a Crate,
    /// Shortest accessible public path per local item `Id` (glob-reexport aware).
    best_path: BTreeMap<u32, Vec<String>>,
    /// Struct `Id` → (owning union `Id`, native variant name).
    payload_index: BTreeMap<u32, (Id, String)>,
    /// Struct `Id`s consumed inline as a newtype variant (never emitted as a
    /// standalone payload/entity).
    newtype_payloads: BTreeSet<u32>,
    /// Type-alias `Id` → its target *struct* `Id` (only for aliases whose target
    /// peels to a local struct — the "entity alias" case, e.g. `Scope`).
    alias_struct: BTreeMap<u32, Id>,
    /// The reflection-root [`ROOT_TYPE_NAME`] struct. Resolved once in [`prepare`]
    /// so a parameter of that type is recognized by `Id` (disambiguation-invariant)
    /// rather than by name, and injected from context instead of taken from Python.
    analysis_id: Option<Id>,
    /// The shared `fpp_ast` grammar classification (an owned snapshot): an
    /// `fpp_ast::X` reference is resolved against this, not a name prefix.
    ast: AstClass,
    /// Leaf enums referenced from the `fpp_ast` arm (`leaf(crate::ast::X)`),
    /// cross-fed back into the AST partition so `ast/defs.rs` mirrors each one.
    used_ast_leaves: BTreeSet<String>,
    /// Diagnostics (types skipped, with reasons) — logged to stderr at the end.
    skips: Vec<String>,
}

impl<'a> Ctx<'a> {
    /// Leaf enums the semantic layer references via `leaf(crate::ast::X)`; fed to
    /// the AST partition's `register_used_leaf` after reflection.
    pub fn used_ast_leaves(&self) -> &BTreeSet<String> {
        &self.used_ast_leaves
    }
    /// Reflection diagnostics accumulated during the closure (field/method drops).
    pub fn skips(&self) -> &[String] {
        &self.skips
    }
    fn item(&self, id: Id) -> Option<&Item> {
        self.krate.index.get(&id)
    }
    fn summary(&self, id: Id) -> Option<&ItemSummary> {
        self.krate.paths.get(&id)
    }
    /// The origin crate name of a referenced `Id` (`"fpp_analysis"` for local).
    fn crate_name(&self, id: Id) -> Option<String> {
        let s = self.summary(id)?;
        if s.crate_id == 0 {
            Some("fpp_analysis".to_string())
        } else {
            self.krate
                .external_crates
                .get(&s.crate_id)
                .map(|c| c.name.clone())
        }
    }
    fn def_path(&self, id: Id) -> Option<&Vec<String>> {
        self.summary(id).map(|s| &s.path)
    }
    fn kind(&self, id: Id) -> Option<ItemKind> {
        self.summary(id).map(|s| s.kind)
    }
    fn is_local(&self, id: Id) -> bool {
        self.summary(id).map(|s| s.crate_id == 0).unwrap_or(false)
    }
    /// The shortest accessible public path (`fpp_analysis::…`); falls back to the
    /// canonical definition path.
    fn native_path(&self, id: Id) -> String {
        if let Some(p) = self.best_path.get(&id.0) {
            return p.join("::");
        }
        self.def_path(id).map(|p| p.join("::")).unwrap_or_default()
    }
    fn last_segment(&self, id: Id) -> String {
        self.def_path(id)
            .and_then(|p| p.last().cloned())
            .unwrap_or_else(|| format!("Item{}", id.0))
    }
    /// The CamelCased last *module* segment of the definition path (for
    /// name-collision disambiguation), e.g. `transition_graph` → `TransitionGraph`.
    fn module_prefix(&self, id: Id) -> String {
        let p = match self.def_path(id) {
            Some(p) if p.len() >= 2 => p,
            _ => return String::new(),
        };
        camel_case(&p[p.len() - 2])
    }
}

fn camel_case(s: &str) -> String {
    s.split('_')
        .filter(|seg| !seg.is_empty())
        .map(|seg| {
            let mut c = seg.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Shortest-public-path BFS (glob-reexport aware)
// ---------------------------------------------------------------------------

fn build_best_paths(krate: &Crate) -> BTreeMap<u32, Vec<String>> {
    let mut best: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    let mut seen: BTreeSet<u32> = BTreeSet::new();
    let mut q: VecDeque<(Id, Vec<String>)> = VecDeque::new();
    q.push_back((krate.root, vec!["fpp_analysis".to_string()]));

    let module_children = |id: Id| -> Vec<Id> {
        match krate.index.get(&id).map(|it| &it.inner) {
            Some(ItemEnum::Module(m)) => m.items.clone(),
            _ => Vec::new(),
        }
    };
    let is_pub = |id: Id| -> bool {
        matches!(
            krate.index.get(&id).map(|it| &it.visibility),
            Some(Visibility::Public)
        )
    };

    while let Some((mid, prefix)) = q.pop_front() {
        if !seen.insert(mid.0) {
            continue;
        }
        best.entry(mid.0).or_insert_with(|| prefix.clone());
        for cid in module_children(mid) {
            let Some(ci) = krate.index.get(&cid) else {
                continue;
            };
            match &ci.inner {
                ItemEnum::Use(u) => {
                    let Some(tgt) = u.id else { continue };
                    if u.is_glob {
                        // Glob re-export: bring the target module's public items
                        // into `prefix` (private target modules still expose their
                        // public items this way).
                        for gc in module_children(tgt) {
                            let Some(gci) = krate.index.get(&gc) else {
                                continue;
                            };
                            if !matches!(gci.visibility, Visibility::Public) {
                                continue;
                            }
                            if matches!(gci.inner, ItemEnum::Use(_)) {
                                continue;
                            }
                            let Some(nm) = &gci.name else { continue };
                            let mut newp = prefix.clone();
                            newp.push(nm.clone());
                            if matches!(gci.inner, ItemEnum::Module(_)) {
                                q.push_back((gc, newp.clone()));
                            }
                            insert_shorter(&mut best, gc.0, newp);
                        }
                    } else {
                        let mut newp = prefix.clone();
                        newp.push(u.name.clone());
                        if matches!(
                            krate.index.get(&tgt).map(|it| &it.inner),
                            Some(ItemEnum::Module(_))
                        ) {
                            q.push_back((tgt, newp.clone()));
                        }
                        insert_shorter(&mut best, tgt.0, newp);
                    }
                }
                ItemEnum::Module(_) if is_pub(cid) => {
                    let mut newp = prefix.clone();
                    newp.push(ci.name.clone().unwrap_or_default());
                    q.push_back((cid, newp));
                }
                _ if is_pub(cid) => {
                    if let Some(nm) = &ci.name {
                        let mut newp = prefix.clone();
                        newp.push(nm.clone());
                        insert_shorter(&mut best, cid.0, newp);
                    }
                }
                _ => {}
            }
        }
    }
    best
}

fn insert_shorter(best: &mut BTreeMap<u32, Vec<String>>, id: u32, p: Vec<String>) {
    match best.get(&id) {
        Some(existing) if existing.len() <= p.len() => {}
        _ => {
            best.insert(id, p);
        }
    }
}

// ---------------------------------------------------------------------------
// Small rustdoc helpers
// ---------------------------------------------------------------------------

/// Whether `imp` is an impl **on** `id` — i.e. `id` is the impl's `for` type.
///
/// Load-bearing filter: rustdoc lists an impl under `Struct::impls` / `Enum::impls`
/// whenever the type appears anywhere in the impl's *trait* generic arguments, not
/// only when it is the `for` type. `fpp_analysis` has several
/// `impl UseAnalysisPass<'ast, Analysis> for CheckUses<'ast>`-shaped pass impls, so
/// `Analysis`'s `impls` list carries `CheckUses`' methods. Reflecting one of those
/// would emit `self.data.analysis.<method>(…)` for a method `Analysis` does not
/// have — a hard compile break in the generated crate. Every `X::impls` walk in
/// this module therefore goes through here.
fn impl_is_for(imp: &rustdoc_types::Impl, id: Id) -> bool {
    // The `for_` type is matched WITHOUT peeling `Box`/`Arc`/`Rc`: an impl on
    // `Arc<X>` is not an impl on `X`, and `peel_wrappers` keys on the last path
    // segment with no locality check — so peeling here would make a local type named
    // `Arc` (this crate has `transition_graph::Arc`) stop matching its own impls the
    // moment it gained a type parameter, silently emptying its method/trait set.
    match &imp.for_ {
        Type::ResolvedPath(p) => p.id == id,
        // An impl written `impl Trait for Self` inside the type's own scope.
        Type::Generic(g) => g == "Self",
        _ => false,
    }
}

/// The `Impl`s declared **on** `id` (inherent + trait), foreign trait-arg impls
/// filtered out by [`impl_is_for`].
fn impls_on(ctx: &Ctx, id: Id) -> Vec<rustdoc_types::Impl> {
    let impl_ids: Vec<Id> = match ctx.item(id).map(|it| &it.inner) {
        Some(ItemEnum::Struct(s)) => s.impls.clone(),
        Some(ItemEnum::Enum(e)) => e.impls.clone(),
        _ => Vec::new(),
    };
    impl_ids
        .into_iter()
        .filter_map(|iid| match ctx.item(iid).map(|it| &it.inner) {
            Some(ItemEnum::Impl(imp)) if impl_is_for(imp, id) => Some(imp.clone()),
            _ => None,
        })
        .collect()
}

fn as_struct(it: &Item) -> Option<&Struct> {
    match &it.inner {
        ItemEnum::Struct(s) => Some(s),
        _ => None,
    }
}
fn as_enum(it: &Item) -> Option<&Enum> {
    match &it.inner {
        ItemEnum::Enum(e) => Some(e),
        _ => None,
    }
}
fn as_variant(it: &Item) -> Option<&Variant> {
    match &it.inner {
        ItemEnum::Variant(v) => Some(v),
        _ => None,
    }
}
fn struct_field_ty(it: &Item) -> Option<&Type> {
    match &it.inner {
        ItemEnum::StructField(t) => Some(t),
        _ => None,
    }
}
fn is_public(it: &Item) -> bool {
    matches!(it.visibility, Visibility::Public)
}

/// The type args of a resolved path (only `GenericArg::Type`s).
fn path_type_args(args: &Option<Box<GenericArgs>>) -> Vec<&Type> {
    match args.as_deref() {
        Some(GenericArgs::AngleBracketed { args, .. }) => args
            .iter()
            .filter_map(|a| match a {
                GenericArg::Type(t) => Some(t),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

const WRAPPERS: &[&str] = &["Box", "Arc", "Rc"];
const LISTS: &[&str] = &["Vec", "VecDeque", "HashSet", "BTreeSet", "FxHashSet"];
const MAPS: &[&str] = &["HashMap", "BTreeMap", "FxHashMap", "IndexMap"];

/// The reflection-root struct in `fpp_analysis`: the entry point of the transitive
/// closure. Every reflected union / entity / payload / leaf enum is reachable from
/// this struct's public fields + eligible methods.
const ROOT_TYPE_NAME: &str = "Analysis";

/// The `fpp_analysis` newtype bridging to the `fpp_ast` [`DEF_MODULE_AST`] node: it
/// is reflected as an `astdef(DefModule)`, never as a standalone payload/entity.
const DEF_MODULE_STUB: &str = "DefModuleStub";
/// The `fpp_ast` node [`DEF_MODULE_STUB`] resolves to.
const DEF_MODULE_AST: &str = "DefModule";

/// Bare type names that always get their CamelCased last-module-segment prefix
/// (even when unique among emitted classes), because the bare form shadows or
/// confuses a type already living in the generated module (e.g.
/// `transition_graph::Node` → `TransitionGraphNode`, `transition_graph::Arc` →
/// `TransitionGraphArc`, and `FormatReplacementKind::Default` → a subclass
/// `FormatReplacementKindDefault` so it does not shadow `std::default::Default`).
///
/// `Enum` and `DefPort` are semantic-layer names that collide with *other things
/// in the same emitted Python module*, invisible to the emitted-class uniqueness
/// pass: `Enum` (the `Symbol::Enum` variant) shadows the `enum.Enum` the stub
/// imports as the base of every leaf-enum mirror, and `DefPort` (the
/// `PortInstanceType::DefPort` variant) shadows the `fpp_ast` `DefPort` node
/// wrapper registered by the sibling AST bindings. Prefixing yields `SymbolEnum`
/// / `PortInstanceTypeDefPort`.
const RESERVED_TYPE_NAMES: &[&str] = &[
    "Arc", "Box", "Rc", "Node", "Cell", "RefCell", "Ref", "Cow", "Weak", "Default", "Enum",
    "DefPort",
];

/// Peel `Box`/`Arc`/`Rc` (transparent) → the innermost referenced type.
fn peel_wrappers(t: &Type) -> &Type {
    if let Type::ResolvedPath(p) = t {
        let last = p.path.rsplit("::").next().unwrap_or(&p.path);
        if WRAPPERS.contains(&last)
            && let Some(inner) = path_type_args(&p.args).into_iter().next()
        {
            return peel_wrappers(inner);
        }
    }
    t
}

// ---------------------------------------------------------------------------
// Payload index + alias index (built once, over ALL local items)
// ---------------------------------------------------------------------------

fn enum_variant_defs<'a>(ctx: &'a Ctx, e: &'a Enum) -> Vec<(&'a Item, &'a Variant)> {
    e.variants
        .iter()
        .filter_map(|vid| {
            let it = ctx.item(*vid)?;
            let v = as_variant(it)?;
            Some((it, v))
        })
        .collect()
}

/// Owned `(native variant name, Variant)` pairs — releases the borrow on `ctx` so
/// the variants can be classified with `&mut ctx`.
fn owned_variants(ctx: &Ctx, enum_id: Id) -> Vec<(String, Variant)> {
    let Some(e) = ctx.item(enum_id).and_then(as_enum) else {
        return Vec::new();
    };
    e.variants
        .iter()
        .filter_map(|vid| {
            let it = ctx.item(*vid)?;
            let v = as_variant(it)?;
            Some((it.name.clone().unwrap_or_default(), v.clone()))
        })
        .collect()
}

/// A single-field tuple variant's inner (peeled) resolved-struct `Id`, if any.
fn single_tuple_local_struct(ctx: &Ctx, v: &Variant) -> Option<Id> {
    let VariantKind::Tuple(fields) = &v.kind else {
        return None;
    };
    if fields.len() != 1 {
        return None;
    }
    let fid = fields[0]?;
    let ft = struct_field_ty(ctx.item(fid)?)?;
    let peeled = peel_wrappers(ft);
    if let Type::ResolvedPath(p) = peeled
        && ctx.is_local(p.id)
        && matches!(ctx.kind(p.id), Some(ItemKind::Struct))
    {
        return Some(p.id);
    }
    None
}

fn is_newtype_struct(ctx: &Ctx, sid: Id) -> bool {
    match ctx.item(sid).and_then(as_struct) {
        Some(s) => matches!(&s.kind, StructKind::Tuple(fs) if fs.len() == 1),
        None => false,
    }
}

fn build_payload_index(ctx: &mut Ctx) {
    // Deterministic order: iterate local enums by native path.
    let mut enum_ids: Vec<Id> = ctx
        .krate
        .index
        .values()
        .filter(|it| matches!(it.inner, ItemEnum::Enum(_)) && ctx.is_local(it.id))
        .map(|it| it.id)
        .collect();
    enum_ids.sort_by_key(|id| ctx.native_path(*id));

    let mut index: BTreeMap<u32, (Id, String)> = BTreeMap::new();
    let mut newtypes: BTreeSet<u32> = BTreeSet::new();
    for eid in enum_ids {
        let e = ctx.item(eid).and_then(as_enum).unwrap();
        for (vit, v) in enum_variant_defs(ctx, e) {
            if let Some(sid) = single_tuple_local_struct(ctx, v) {
                // The `DefModuleStub` bridge is an astdef, never a payload.
                if ctx.last_segment(sid) == DEF_MODULE_STUB {
                    continue;
                }
                let vname = vit.name.clone().unwrap_or_default();
                index.entry(sid.0).or_insert((eid, vname));
                if is_newtype_struct(ctx, sid) {
                    newtypes.insert(sid.0);
                }
            }
        }
    }
    ctx.payload_index = index;
    ctx.newtype_payloads = newtypes;
}

fn build_alias_index(ctx: &mut Ctx) {
    let mut map: BTreeMap<u32, Id> = BTreeMap::new();
    let alias_ids: Vec<Id> = ctx
        .krate
        .index
        .values()
        .filter(|it| matches!(it.inner, ItemEnum::TypeAlias(_)) && ctx.is_local(it.id))
        .map(|it| it.id)
        .collect();
    for aid in alias_ids {
        if let Some(sid) = resolve_alias_to_struct(ctx, aid) {
            map.insert(aid.0, sid);
        }
    }
    ctx.alias_struct = map;
}

/// Resolve a reference to a **non-generic** local type alias to its target type
/// (following chains). `None` when `t` is not such an alias.
///
/// Only argument-free aliases are followed: rustdoc already expands a generic alias
/// applied to arguments, so an alias that still carries type args here would need
/// substitution this reflection does not model.
fn resolve_type_alias(ctx: &Ctx, t: &Type) -> Option<Type> {
    let Type::ResolvedPath(p) = t else {
        return None;
    };
    if !ctx.is_local(p.id) || !path_type_args(&p.args).is_empty() {
        return None;
    }
    let ItemEnum::TypeAlias(ta) = &ctx.item(p.id)?.inner else {
        return None;
    };
    let target = ta.type_.clone();
    Some(resolve_type_alias(ctx, &target).unwrap_or(target))
}

/// Follow a type-alias chain; if it peels to a local struct, return that struct's
/// `Id` (the "entity alias" case). Container/primitive targets return `None`.
fn resolve_alias_to_struct(ctx: &Ctx, alias_id: Id) -> Option<Id> {
    let ItemEnum::TypeAlias(ta) = &ctx.item(alias_id)?.inner else {
        return None;
    };
    let peeled = peel_wrappers(&ta.type_);
    if let Type::ResolvedPath(p) = peeled {
        if !ctx.is_local(p.id) {
            return None;
        }
        match ctx.kind(p.id) {
            Some(ItemKind::Struct) => Some(p.id),
            Some(ItemKind::TypeAlias) => resolve_alias_to_struct(ctx, p.id),
            _ => None,
        }
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

/// Classify a resolved rustdoc `Type` into a [`Shape`], pushing referenced local
/// `Id`s onto `enq` for the transitive closure.
fn classify(ctx: &mut Ctx, t: &Type, enq: &mut Vec<Id>) -> Shape {
    match t {
        Type::Primitive(p) => match p.as_str() {
            "bool" => Shape::Bool,
            "i128" => Shape::I128,
            "f32" | "f64" => Shape::F64,
            "str" => Shape::Str,
            "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" => {
                Shape::Usize
            }
            other => Shape::Skip(format!("primitive {other}")),
        },
        Type::BorrowedRef { type_, .. } => classify(ctx, type_, enq),
        Type::Tuple(elems) if elems.is_empty() => Shape::Unit,
        Type::Tuple(elems) => {
            let shapes: Vec<Shape> = elems.iter().map(|e| classify(ctx, e, enq)).collect();
            if let Some(s) = shapes.iter().find(|s| s.is_skip()) {
                return s.clone();
            }
            Shape::Tuple(shapes)
        }
        Type::ResolvedPath(p) => {
            let last = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
            // Containers (detected by last segment, any crate).
            if WRAPPERS.contains(&last.as_str())
                && let Some(inner) = path_type_args(&p.args).into_iter().next()
            {
                return classify(ctx, inner, enq);
            }
            if last == "Option"
                && let Some(inner) = path_type_args(&p.args).into_iter().next()
            {
                let s = classify(ctx, inner, enq);
                return if s.is_skip() {
                    s
                } else {
                    Shape::Opt(Box::new(s))
                };
            }
            if LISTS.contains(&last.as_str())
                && let Some(inner) = path_type_args(&p.args).into_iter().next()
            {
                let s = classify(ctx, inner, enq);
                return if s.is_skip() {
                    s
                } else {
                    Shape::List(Box::new(s))
                };
            }
            if MAPS.contains(&last.as_str()) {
                let args = path_type_args(&p.args);
                if args.len() >= 2 {
                    let k = classify(ctx, args[0], enq);
                    let v = classify(ctx, args[1], enq);
                    if k.is_skip() {
                        return Shape::Skip(format!("map key: {}", skip_reason(&k)));
                    }
                    if v.is_skip() {
                        return Shape::Skip(format!("map value: {}", skip_reason(&v)));
                    }
                    return Shape::Map(Box::new(k), Box::new(v));
                }
            }
            if last == "String" {
                return Shape::Str;
            }
            // Origin-based classification.
            match ctx.crate_name(p.id).as_deref() {
                Some("fpp_core") => match last.as_str() {
                    "Node" => Shape::Node,
                    "Span" => Shape::Span,
                    _ => Shape::Skip(format!(
                        "fpp_core::{last} (unrecognized fpp_core type; classify handles only Node/Span)"
                    )),
                },
                // Resolve against the REAL grammar partition (Rule R1). Shadowing
                // is applied later, by the driver's normalize phase, once the
                // resolved semantic names are known.
                Some("fpp_ast") => match ctx.ast.classify_ast_ref(&last) {
                    AstRef::AstDef(name) => Shape::AstDef(name),
                    AstRef::Leaf(name) => {
                        ctx.used_ast_leaves.insert(name.clone());
                        Shape::LeafAst(name)
                    }
                    AstRef::Skip(reason) => Shape::Skip(reason),
                },
                Some("fpp_analysis") => classify_local(ctx, p.id, &last, enq),
                Some(other) => Shape::Skip(format!("{other}::{last}")),
                None => Shape::Skip(format!("unresolved {last}")),
            }
        }
        Type::Generic(g) => Shape::Skip(format!("generic {g}")),
        // `&[T]` converts exactly like `Vec<T>`: the `list` emitter only iterates.
        Type::Slice(inner) => {
            let s = classify(ctx, inner, enq);
            if s.is_skip() {
                s
            } else {
                Shape::List(Box::new(s))
            }
        }
        Type::Array { .. } => Shape::Skip("array".into()),
        Type::QualifiedPath { name, .. } => Shape::Skip(format!("qualified path {name}")),
        _ => Shape::Skip("unrecognized type".into()),
    }
}

fn skip_reason(s: &Shape) -> String {
    match s {
        Shape::Skip(r) => r.clone(),
        _ => "?".into(),
    }
}

/// A short human-readable description of a `Type` for skip diagnostics (the last
/// path segment for resolved types, the primitive name, etc.).
fn type_desc(t: &Type) -> String {
    match t {
        Type::Primitive(p) => p.clone(),
        Type::BorrowedRef { type_, .. } => format!("&{}", type_desc(type_)),
        Type::ResolvedPath(p) => p.path.rsplit("::").next().unwrap_or(&p.path).to_string(),
        Type::Tuple(_) => "tuple".into(),
        Type::Slice(_) => "slice".into(),
        Type::Array { .. } => "array".into(),
        Type::Generic(g) => g.clone(),
        Type::QualifiedPath { name, .. } => name.clone(),
        _ => "?".into(),
    }
}

/// Classify a local (`fpp_analysis`) resolved type by its `Id`.
fn classify_local(ctx: &mut Ctx, id: Id, last: &str, enq: &mut Vec<Id>) -> Shape {
    // The `DefModuleStub` bridge → the real `DefModule` AST node.
    if last == DEF_MODULE_STUB {
        return Shape::AstDef(DEF_MODULE_AST.into());
    }
    match ctx.kind(id) {
        Some(ItemKind::Enum) => {
            enq.push(id);
            if enum_all_unit(ctx, id) {
                Shape::LeafEnum(id)
            } else {
                Shape::Union(id)
            }
        }
        Some(ItemKind::Struct) => {
            if let Some((uid, variant)) = ctx.payload_index.get(&id.0).cloned() {
                enq.push(uid);
                Shape::Rewrap(uid, variant)
            } else if impls_clone(ctx, id) {
                enq.push(id);
                Shape::Entity(id)
            } else {
                // The clone-entity handle stores the native by value and clones it
                // (see the macro's `entity`), so a struct without `Clone` cannot be
                // emitted — skip it (logged) rather than emit code that won't build.
                Shape::Skip(format!("fpp_analysis::{last} (struct has no Clone impl)"))
            }
        }
        Some(ItemKind::TypeAlias) => {
            // An alias to a local struct → an "entity alias" (reflected under the
            // alias's own name/path); anything else → inline the target. The alias's
            // native type is (transparently) the target struct, so the clone-entity
            // handle still requires the target to impl `Clone`.
            if let Some(target) = ctx.alias_struct.get(&id.0).copied() {
                if impls_clone(ctx, target) {
                    enq.push(id);
                    Shape::Entity(id)
                } else {
                    Shape::Skip(format!("alias {last} (target struct has no Clone impl)"))
                }
            } else if let ItemEnum::TypeAlias(ta) = &ctx.item(id).unwrap().inner {
                let target = ta.type_.clone();
                classify(ctx, &target, enq)
            } else {
                Shape::Skip(format!("alias {last}"))
            }
        }
        other => Shape::Skip(format!("fpp_analysis::{last} ({other:?})")),
    }
}

fn enum_all_unit(ctx: &Ctx, id: Id) -> bool {
    match ctx.item(id).and_then(as_enum) {
        Some(e) => e.variants.iter().all(|vid| {
            matches!(
                ctx.item(*vid).and_then(as_variant).map(|v| &v.kind),
                Some(VariantKind::Plain)
            )
        }),
        None => true,
    }
}

// ---------------------------------------------------------------------------
// Method reflection
// ---------------------------------------------------------------------------

/// Reflect eligible methods for a type from its inherent impls and from trait
/// impls whose trait is defined in `fpp_analysis`. `owner` is the reflected type;
/// `impl_owner` is the type whose `impls` list is walked (the alias target for an
/// entity alias). Deduped by name, sorted by name.
fn methods_for(ctx: &mut Ctx, impl_owner: Id, enq: &mut Vec<Id>) -> Vec<MethodDef> {
    let mut out: BTreeMap<String, MethodDef> = BTreeMap::new();
    for imp in impls_on(ctx, impl_owner) {
        // Inherent impls, or trait impls whose trait is defined in `fpp_analysis`.
        let is_trait = imp.trait_.is_some();
        let mut trait_path = None;
        if let Some(tr) = &imp.trait_ {
            if ctx.crate_name(tr.id).as_deref() != Some("fpp_analysis") {
                continue;
            }
            trait_path = Some(ctx.native_path(tr.id));
        }
        for mid in &imp.items {
            // Inherent methods must be `pub`; trait-impl method items carry
            // `Default` visibility (they are accessible via the public trait).
            let Some(mut m) = classify_method(ctx, impl_owner, *mid, !is_trait, enq) else {
                continue;
            };
            m.trait_path = trait_path.clone();
            out.entry(m.name.clone()).or_insert(m);
        }
    }
    out.into_values().collect()
}

fn classify_method(
    ctx: &mut Ctx,
    owner: Id,
    mid: Id,
    require_public: bool,
    enq: &mut Vec<Id>,
) -> Option<MethodDef> {
    // Snapshot the (owned) signature so the immutable borrow of `ctx` is dropped
    // before the mutable `classify` call on the return type.
    let (name, inputs, output) = {
        let it = ctx.item(mid)?;
        if require_public && !is_public(it) {
            return None;
        }
        let name = it.name.clone()?;
        let ItemEnum::Function(f) = &it.inner else {
            return None;
        };
        // Skip type/const generic params (lifetimes are fine).
        for pd in &f.generics.params {
            if matches!(
                pd.kind,
                GenericParamDefKind::Type { .. } | GenericParamDefKind::Const { .. }
            ) {
                return None;
            }
        }
        (name, f.sig.inputs.clone(), f.sig.output.clone())
    };

    let first = inputs.first()?;
    let (assoc, rest) = receiver(owner, first)?;

    // Every remaining arg must be a marshallable argkind. A param outside the
    // supported vocabulary drops the whole method — logged (matching the
    // struct/enum skip style) rather than dropped silently.
    let mut params: Vec<(String, ArgSpec)> = Vec::new();
    for (pname, pty) in &inputs[rest..] {
        match arg_spec(ctx, pty, enq) {
            Ok(spec) => params.push((pname.clone(), spec)),
            Err(reason) => {
                let owner_name = ctx.last_segment(owner);
                let desc = type_desc(pty);
                ctx.skips.push(format!(
                    "  skip {owner_name}.{name}(): unsupported param `{pname}: {desc}` ({reason})"
                ));
                return None;
            }
        }
    }

    // A `Result<T, E>` return is marshalled as a raising method: the Python-facing
    // shape is `T`, and `E` becomes the raised exception (rendered with `{:?}`, so
    // `E` must impl `Debug`).
    let out = output.as_ref()?;
    let (out, throws) = match split_result(ctx, out) {
        Some((ok, err_desc, err_debug)) => {
            if !err_debug {
                let owner_name = ctx.last_segment(owner);
                ctx.skips.push(format!(
                    "  skip {owner_name}.{name}(): Result error type `{err_desc}` has no Debug \
                     impl (nothing to render into the raised exception)"
                ));
                return None;
            }
            (ok, Some(err_desc))
        }
        None => (out.clone(), None),
    };

    // Return type must classify to a non-skip shape; a computed `span` (needs the
    // live context outside a plain getter) is skipped conservatively. Either drop
    // is logged (matching the param-drop style) rather than silent — a method with
    // a valid receiver + params but an unconvertible return is a real omission.
    let ret = classify(ctx, &out, enq);
    if ret.is_skip() || matches!(ret, Shape::Span) {
        let owner_name = ctx.last_segment(owner);
        let reason = if matches!(ret, Shape::Span) {
            "computed span (needs the live context outside a getter)".to_string()
        } else {
            skip_reason(&ret)
        };
        ctx.skips.push(format!(
            "  skip {owner_name}.{name}(): unsupported return ({reason})"
        ));
        return None;
    }
    // Rule R2: an `fpp_ast` node returned BY VALUE from an analysis method is
    // synthesized fresh (its `node_id` is never recorded in the walk), so an
    // `astdef` getter over it would raise at runtime. Only fields / variant
    // payloads carry recorded walk nodes; drop the method (logged).
    if shape_contains_astdef(&ret) {
        let owner_name = ctx.last_segment(owner);
        ctx.skips.push(format!(
            "  skip {owner_name}.{name}(): fpp_ast node returned by value from an analysis \
             method is built fresh (not a recorded walk node); not runtime-safe as astdef"
        ));
        return None;
    }
    Some(MethodDef {
        name,
        assoc,
        params,
        ret,
        // Only the OUTERMOST reference is recorded: a nested one
        // (`Option<&Leaf>`) reaches the shape through the container's own
        // conversion, which cannot be told to expect one fewer `&`. No such
        // signature is reflected today, and one would fail to compile rather than
        // marshal wrongly.
        ret_ref: matches!(out, Type::BorrowedRef { .. }),
        throws,
        trait_path: None,
    })
}

/// Split a `Result<T, E>` return into `(T, E's display name, whether E impls Debug)`.
/// `None` for any non-`Result` return.
///
/// `fpp_analysis` spells these through aliases. A *generic* alias applied to an
/// argument (`SemanticResult<i128>`) is already expanded to `Result<…>` by rustdoc,
/// but a *non-generic* one (`MathResult`, `TypeConversionResult`) stays a path to the
/// alias item — so aliases are resolved here first rather than listed in a table.
fn split_result(ctx: &Ctx, t: &Type) -> Option<(Type, String, bool)> {
    let resolved = resolve_type_alias(ctx, t);
    let t = resolved.as_ref().unwrap_or(t);
    let Type::ResolvedPath(p) = t else {
        return None;
    };
    // A local type named `Result` would not be `core::Result` (see the container
    // guard in `arg_inner` for the same hazard).
    if ctx.is_local(p.id) || p.path.rsplit("::").next().unwrap_or(&p.path) != "Result" {
        return None;
    }
    let args = path_type_args(&p.args);
    if args.len() != 2 {
        return None;
    }
    let err = args[1];
    let debug = match err {
        Type::ResolvedPath(ep) if ctx.is_local(ep.id) => impls_debug(ctx, ep.id),
        // A non-local error type is not reflected here, so its `Debug` impl cannot
        // be proven from this crate's rustdoc JSON.
        _ => false,
    };
    Some((args[0].clone(), type_desc(err), debug))
}

/// Whether a return shape contains an `fpp_ast` walked node anywhere (Rule R2),
/// recursing through `opt`/`list`/`map`/`tuple`.
fn shape_contains_astdef(s: &Shape) -> bool {
    match s {
        Shape::AstDef(_) => true,
        Shape::Opt(i) | Shape::List(i) => shape_contains_astdef(i),
        Shape::Map(k, v) => shape_contains_astdef(k) || shape_contains_astdef(v),
        Shape::Tuple(v) => v.iter().any(shape_contains_astdef),
        _ => false,
    }
}

/// Interpret the first input as a receiver. Returns `(assoc, rest_start_index)`.
fn receiver(owner: Id, first: &(String, Type)) -> Option<(bool, usize)> {
    let (fname, ftype) = first;
    if fname == "self" {
        // `&self` / `&Arc<Self>` / `&Self`: a non-mut borrowed ref.
        match ftype {
            Type::BorrowedRef {
                is_mutable: false, ..
            } => Some((false, 1)),
            _ => None,
        }
    } else {
        // Associated receiver: `&Arc<Owner>` / `&Owner` / `&Self`.
        let Type::BorrowedRef {
            is_mutable: false,
            type_,
            ..
        } = ftype
        else {
            return None;
        };
        match peel_wrappers(type_) {
            Type::Generic(g) if g == "Self" => Some((true, 1)),
            Type::ResolvedPath(p) if p.id == owner => Some((true, 1)),
            _ => None,
        }
    }
}

/// Classify a method parameter into an [`ArgSpec`], pushing referenced local `Id`s
/// onto `enq` so a type reachable ONLY as a parameter still gets a Python wrapper
/// (without one the param could not be supplied). `Err` carries the skip reason.
fn arg_spec(ctx: &mut Ctx, t: &Type, enq: &mut Vec<Id>) -> Result<ArgSpec, String> {
    // Preserve owned-vs-borrowed: peeling `&` loses it, and an owned param must
    // marshal differently from a borrowed one (the wrapper only ever *lends* its
    // native, so an owned param takes a clone).
    let (borrowed, base) = match t {
        // A `&mut` param would need the wrapper to hand out a mutable native. The
        // whole model is `frozen`/read-only, so this can never be supplied — and
        // silently treating it as a shared borrow would emit non-compiling code.
        Type::BorrowedRef {
            is_mutable: true, ..
        } => return Err(format!("&mut {} (read-only wrappers)", type_desc(t))),
        Type::BorrowedRef { type_, .. } => (true, type_.as_ref()),
        other => (false, other),
    };
    let arg = arg_inner(ctx, base, borrowed, enq)?;
    // Scalars encode their own pass form in the token, so they are never `ref`.
    let borrowed = borrowed && !matches!(arg, Arg::Scalar(_) | Arg::Analysis);
    Ok(ArgSpec { arg, borrowed })
}

/// Classify the (already `&`-peeled) parameter type. `borrowed` only distinguishes
/// `&str` from `String`; every nested position is by value.
fn arg_inner(ctx: &mut Ctx, t: &Type, borrowed: bool, enq: &mut Vec<Id>) -> Result<Arg, String> {
    match t {
        Type::Primitive(p) => match p.as_str() {
            "bool" => Ok(Arg::Scalar("bool")),
            "i128" => Ok(Arg::Scalar("i128")),
            // `&str` is inherently borrowed (unsized), so it is always `str`.
            "str" => Ok(Arg::Scalar("str")),
            "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" => {
                Ok(Arg::Scalar("usize"))
            }
            other => Err(format!("primitive {other}")),
        },
        // `&[T]` is fed from a Python list: the `list` argkind builds a `Vec<T>`, and
        // `&Vec<T>` deref-coerces to `&[T]`. That coercion is what makes a slice
        // work, so it only applies behind the top-level `&` — an unsized slice nested
        // in a container (`Arc<[T]>`) would silently become `Arc<Vec<T>>`.
        Type::Slice(inner) if borrowed => {
            Ok(Arg::List(Box::new(arg_inner(ctx, inner, false, enq)?)))
        }
        Type::Slice(_) => Err("slice nested in a container".into()),
        Type::ResolvedPath(p) => {
            let last = p.path.rsplit("::").next().unwrap_or(&p.path).to_string();
            if last == "String" {
                return Ok(Arg::Scalar(if borrowed { "str" } else { "string" }));
            }
            let args = path_type_args(&p.args);
            // Containers, by last segment — but only for types from OTHER crates.
            // `fpp_analysis` has its own `transition_graph::Arc` (a semantic union),
            // and a bare last-segment match would read it as `std::sync::Arc` and
            // then fail looking for a type argument. `Box`/`Rc` are deliberately not
            // peeled: unlike a getter shape (where the wrapper is transparent) a
            // parameter must be rebuilt in exactly the native's own container, and no
            // `fpp_analysis` signature uses either today.
            if !ctx.is_local(p.id) {
                match last.as_str() {
                    "Arc" => {
                        let inner = args.first().ok_or("Arc<?>")?;
                        return Ok(Arg::Arc(Box::new(arg_inner(ctx, inner, false, enq)?)));
                    }
                    "Option" => {
                        let inner = args.first().ok_or("Option<?>")?;
                        return Ok(Arg::Opt(Box::new(arg_inner(ctx, inner, false, enq)?)));
                    }
                    "Vec" | "VecDeque" => {
                        let inner = args.first().ok_or("Vec<?>")?;
                        return Ok(Arg::List(Box::new(arg_inner(ctx, inner, false, enq)?)));
                    }
                    _ => {}
                }
            }
            match ctx.crate_name(p.id).as_deref() {
                Some("fpp_core") => match last.as_str() {
                    "Node" => Ok(Arg::Node),
                    "Span" => Ok(Arg::Span),
                    _ => Err(format!("fpp_core::{last} (no wrapper supplies it)")),
                },
                // Resolve against the REAL grammar partition, exactly as `classify`
                // does for a getter shape. Only a recorded walk node has a
                // `crate::ast::<Name>` wrapper able to lend the native back.
                Some("fpp_ast") => match ctx.ast.classify_ast_ref(&last) {
                    AstRef::AstDef(name) => Ok(Arg::AstNode(name)),
                    AstRef::Leaf(name) => Err(format!(
                        "fpp_ast::{name} (leaf-enum mirrors are one-way: no Python -> \
                         native conversion)"
                    )),
                    AstRef::Skip(reason) => Err(reason),
                },
                Some("fpp_analysis") => arg_local(ctx, p.id, &last, borrowed, enq),
                Some(other) => Err(format!("{other}::{last}")),
                None => Err(format!("unresolved {last}")),
            }
        }
        Type::Generic(g) => Err(format!("generic {g}")),
        Type::Array { .. } => Err("array".into()),
        Type::Tuple(_) => Err("tuple".into()),
        Type::QualifiedPath { name, .. } => Err(format!("qualified path {name}")),
        _ => Err("unrecognized type".into()),
    }
}

/// Classify a local (`fpp_analysis`) parameter type by its `Id` — the parameter-side
/// peer of [`classify_local`].
fn arg_local(
    ctx: &mut Ctx,
    id: Id,
    last: &str,
    borrowed: bool,
    enq: &mut Vec<Id>,
) -> Result<Arg, String> {
    // The reflection root is injected from context, never passed from Python.
    if Some(id) == ctx.analysis_id {
        return Ok(Arg::Analysis);
    }
    // NOTE: deliberately no `DEF_MODULE_STUB` -> `astdef(DEF_MODULE_AST)` bridge here,
    // unlike `classify_local`. That mapping works for a getter because the shape only
    // reads `.node_id`, which the stub also has; a parameter would hand the native a
    // real `fpp_ast::DefModule` where it wants a `DefModuleStub`. So the stub falls
    // through below and a parameter of that type is skipped.
    match ctx.kind(id) {
        Some(ItemKind::Enum) => {
            if enum_all_unit(ctx, id) {
                Err(format!(
                    "fpp_analysis::{last} (leaf-enum mirrors are one-way: no Python -> \
                     native conversion)"
                ))
            } else {
                enq.push(id);
                Ok(Arg::Union(id))
            }
        }
        Some(ItemKind::Struct) => {
            if ctx.payload_index.contains_key(&id.0) {
                // A bare payload struct is only reachable in Python as a union
                // subclass, whose stored native is the whole enum; projecting the
                // payload back out would need a variant match per call site.
                Err(format!(
                    "fpp_analysis::{last} (bare union payload; no standalone Python class)"
                ))
            } else if impls_clone(ctx, id) {
                enq.push(id);
                Ok(Arg::Entity(id))
            } else {
                Err(format!("fpp_analysis::{last} (struct has no Clone impl)"))
            }
        }
        Some(ItemKind::TypeAlias) => {
            if let Some(target) = ctx.alias_struct.get(&id.0).copied() {
                if impls_clone(ctx, target) {
                    enq.push(id);
                    Ok(Arg::Entity(id))
                } else {
                    Err(format!("alias {last} (target struct has no Clone impl)"))
                }
            } else if let ItemEnum::TypeAlias(ta) = &ctx.item(id).unwrap().inner {
                // Carry `borrowed` through: the alias is transparent, so a `&Alias`
                // param still wants the borrowed pass form of whatever it resolves to.
                let target = ta.type_.clone();
                arg_inner(ctx, &target, borrowed, enq)
            } else {
                Err(format!("alias {last}"))
            }
        }
        other => Err(format!("fpp_analysis::{last} ({other:?})")),
    }
}

// ---------------------------------------------------------------------------
// Union variant payloadkind
// ---------------------------------------------------------------------------

fn variant_payload(ctx: &mut Ctx, v: &Variant, enq: &mut Vec<Id>) -> VariantPayload {
    match &v.kind {
        VariantKind::Plain => VariantPayload::Unit,
        VariantKind::Struct { fields, .. } => {
            // Enum struct-variant fields are always accessible (rustdoc labels them
            // `Default`, not `Public`) — do not filter by visibility.
            let mut fs = Vec::new();
            for fid in fields {
                let Some(fit) = ctx.item(*fid) else { continue };
                let name = fit.name.clone().unwrap_or_default();
                let ty = struct_field_ty(fit).cloned();
                let sh = match ty {
                    Some(t) => classify(ctx, &t, enq),
                    None => Shape::Skip("no field type".into()),
                };
                fs.push((name, sh));
            }
            VariantPayload::StructVariant(fs)
        }
        VariantKind::Tuple(fields) => {
            if fields.len() != 1 {
                return VariantPayload::Other;
            }
            let Some(fid) = fields[0] else {
                return VariantPayload::Other;
            };
            let ft = match ctx.item(fid).and_then(struct_field_ty) {
                Some(t) => t.clone(),
                None => return VariantPayload::Other,
            };
            let peeled = peel_wrappers(&ft).clone();
            // A local struct payload (not the DefModuleStub bridge).
            if let Type::ResolvedPath(p) = &peeled
                && ctx.is_local(p.id)
                && matches!(ctx.kind(p.id), Some(ItemKind::Struct))
                && ctx.last_segment(p.id) != DEF_MODULE_STUB
            {
                if ctx.newtype_payloads.contains(&p.id.0) {
                    // Single-field tuple struct → inline as a `newtype`.
                    let f0 = newtype_field_shape(ctx, p.id, enq);
                    return VariantPayload::Newtype(f0);
                }
                enq.push(p.id);
                return VariantPayload::Struct(p.id);
            }
            // A single-value variant (scalar / leaf / astdef / union / …).
            VariantPayload::Value(classify(ctx, &ft, enq))
        }
    }
}

fn newtype_field_shape(ctx: &mut Ctx, sid: Id, enq: &mut Vec<Id>) -> Shape {
    let field_id = match ctx.item(sid).and_then(as_struct).map(|s| &s.kind) {
        Some(StructKind::Tuple(fs)) if fs.len() == 1 => fs[0],
        _ => None,
    };
    match field_id
        .and_then(|fid| ctx.item(fid))
        .and_then(struct_field_ty)
    {
        Some(t) => {
            let t = t.clone();
            classify(ctx, &t, enq)
        }
        None => Shape::Skip("newtype field".into()),
    }
}

// ---------------------------------------------------------------------------
// Struct fields
// ---------------------------------------------------------------------------

fn struct_pub_fields(ctx: &mut Ctx, sid: Id, enq: &mut Vec<Id>) -> Vec<(String, Shape)> {
    let kind = match ctx.item(sid).and_then(as_struct).map(|s| s.kind.clone()) {
        Some(k) => k,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    match kind {
        StructKind::Plain { fields, .. } => {
            for fid in fields {
                let Some(fit) = ctx.item(fid) else { continue };
                if !is_public(fit) {
                    continue;
                }
                let name = fit.name.clone().unwrap_or_default();
                let ty = struct_field_ty(fit).cloned();
                let sh = match ty {
                    Some(t) => classify(ctx, &t, enq),
                    None => Shape::Skip("no field type".into()),
                };
                out.push((name, sh));
            }
        }
        StructKind::Tuple(fields) => {
            // Tuple-struct fields are positional (`0`, `1`, …) — not addressable
            // as Python attributes and not valid DSL identifiers. Classify them to
            // drive the closure (so referenced types are still reached), but never
            // emit them; such a struct's usable surface is its methods.
            for fid in &fields {
                let Some(fid) = fid else { continue };
                let Some(fit) = ctx.item(*fid) else { continue };
                if !is_public(fit) {
                    continue;
                }
                if let Some(t) = struct_field_ty(fit).cloned() {
                    let _ = classify(ctx, &t, enq);
                }
            }
        }
        StructKind::Unit => {}
    }
    out
}

/// Whether a type has a no-arg `qualified_name(&self) -> String` inherent/trait
/// method (→ `identity qualified_name`).
fn has_qualified_name(ctx: &Ctx, impl_owner: Id) -> bool {
    for imp in impls_on(ctx, impl_owner) {
        for mid in &imp.items {
            let Some(mi) = ctx.item(*mid) else { continue };
            if mi.name.as_deref() != Some("qualified_name") {
                continue;
            }
            let ItemEnum::Function(f) = &mi.inner else {
                continue;
            };
            // No-arg: only the `&self` receiver.
            if f.sig.inputs.len() == 1 && f.sig.inputs[0].0 == "self" {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Closure
// ---------------------------------------------------------------------------

pub struct Reflected {
    analysis_id: Id,
    analysis_fields: Vec<(String, Shape)>,
    analysis_methods: Vec<MethodDef>,
    pub unions: Vec<UnionDef>,
    pub payloads: Vec<PayloadDef>,
    pub entities: Vec<EntityDef>,
    pub leaf_enums: Vec<LeafEnumDef>,
}

pub fn reflect(ctx: &mut Ctx) -> Reflected {
    let analysis_id = ctx.analysis_id.unwrap_or_else(|| {
        panic!("reflection-root struct `fpp_analysis::{ROOT_TYPE_NAME}` not found")
    });

    let mut reached: BTreeSet<u32> = BTreeSet::new();
    let mut queue: Vec<Id> = Vec::new();
    let push = |reached: &mut BTreeSet<u32>, queue: &mut Vec<Id>, ids: Vec<Id>| {
        for id in ids {
            if reached.insert(id.0) {
                queue.push(id);
            }
        }
    };

    // Seed: Analysis public fields + eligible methods.
    let mut enq = Vec::new();
    let analysis_fields = struct_pub_fields(ctx, analysis_id, &mut enq);
    let analysis_methods = methods_for(ctx, analysis_id, &mut enq);
    push(&mut reached, &mut queue, std::mem::take(&mut enq));

    // Process the closure to a fixpoint.
    while let Some(id) = queue.pop() {
        let mut enq = Vec::new();
        match ctx.item(id).map(|it| &it.inner) {
            Some(ItemEnum::Enum(_)) => {
                // Enqueue variant payloads + method returns.
                for (_native, v) in owned_variants(ctx, id) {
                    let _ = variant_payload(ctx, &v, &mut enq);
                }
                let _ = methods_for(ctx, id, &mut enq);
            }
            Some(ItemEnum::Struct(_)) => {
                let _ = struct_pub_fields(ctx, id, &mut enq);
                let _ = methods_for(ctx, id, &mut enq);
            }
            Some(ItemEnum::TypeAlias(_)) => {
                // Entity alias: reflect the target struct's members.
                if let Some(sid) = ctx.alias_struct.get(&id.0).copied() {
                    let _ = struct_pub_fields(ctx, sid, &mut enq);
                    let _ = methods_for(ctx, sid, &mut enq);
                }
            }
            _ => {}
        }
        push(&mut reached, &mut queue, enq);
    }

    // Partition the reached set into DSL items.
    let mut unions = Vec::new();
    let mut payloads = Vec::new();
    let mut entities = Vec::new();
    let mut leaf_enums = Vec::new();

    let mut ids: Vec<u32> = reached.iter().copied().collect();
    ids.sort();
    for raw in ids {
        let id = Id(raw);
        if id == analysis_id {
            continue;
        }
        match ctx.item(id).map(|it| &it.inner) {
            Some(ItemEnum::Enum(_)) => {
                if enum_all_unit(ctx, id) {
                    leaf_enums.push(build_leaf_enum(ctx, id));
                } else {
                    unions.push(build_union(ctx, id));
                }
            }
            Some(ItemEnum::Struct(_)) => {
                if ctx.newtype_payloads.contains(&id.0) {
                    // Consumed inline as a newtype variant — never emitted.
                    continue;
                }
                if ctx.payload_index.contains_key(&id.0) {
                    payloads.push(build_payload(ctx, id));
                } else {
                    entities.push(build_entity(ctx, id, id));
                }
            }
            Some(ItemEnum::TypeAlias(_)) => {
                if let Some(sid) = ctx.alias_struct.get(&id.0).copied() {
                    entities.push(build_entity(ctx, id, sid));
                }
            }
            _ => {}
        }
    }

    Reflected {
        analysis_id,
        analysis_fields,
        analysis_methods,
        unions,
        payloads,
        entities,
        leaf_enums,
    }
}

fn find_analysis(ctx: &Ctx) -> Option<Id> {
    ctx.krate
        .index
        .values()
        .filter(|it| {
            it.name.as_deref() == Some(ROOT_TYPE_NAME)
                && matches!(it.inner, ItemEnum::Struct(_))
                && ctx.is_local(it.id)
        })
        .map(|it| it.id)
        // The crate-level `Analysis` (def path `fpp_analysis::analysis::Analysis`).
        .find(|id| {
            ctx.def_path(*id)
                .map(|p| p.last().map(String::as_str) == Some(ROOT_TYPE_NAME))
                .unwrap_or(false)
        })
}

fn build_union(ctx: &mut Ctx, id: Id) -> UnionDef {
    let mut variants = Vec::new();
    let mut enq = Vec::new();
    for (native, v) in owned_variants(ctx, id) {
        let payload = variant_payload(ctx, &v, &mut enq);
        variants.push(VariantDef { native, payload });
    }
    let methods = methods_for(ctx, id, &mut enq);
    UnionDef {
        id,
        variants,
        methods,
    }
}

fn build_payload(ctx: &mut Ctx, id: Id) -> PayloadDef {
    let mut enq = Vec::new();
    let fields = struct_pub_fields(ctx, id, &mut enq);
    let mut methods = methods_for(ctx, id, &mut enq);
    drop_field_shadowing_methods(ctx, &ctx.last_segment(id), &fields, &mut methods);
    PayloadDef {
        id,
        fields,
        methods,
    }
}

/// Drop methods whose name collides with a struct field name (a field getter and
/// a method getter cannot coexist in one `#[pymethods]` block). The field is the
/// canonical data member and wins; the shadowed method is dropped (logged).
fn drop_field_shadowing_methods(
    ctx: &mut Ctx,
    owner: &str,
    fields: &[(String, Shape)],
    methods: &mut Vec<MethodDef>,
) {
    let field_names: BTreeSet<String> = fields.iter().map(|(n, _)| n.clone()).collect();
    let mut dropped: Vec<String> = Vec::new();
    methods.retain(|m| {
        if field_names.contains(&m.name) {
            dropped.push(m.name.clone());
            false
        } else {
            true
        }
    });
    for m in dropped {
        ctx.skips.push(format!(
            "  skip {owner}.{m}() (method shadowed by field of the same name)"
        ));
    }
}

fn build_entity(ctx: &mut Ctx, id: Id, impl_id: Id) -> EntityDef {
    let mut enq = Vec::new();
    let fields = struct_pub_fields(ctx, impl_id, &mut enq);
    let mut methods = methods_for(ctx, impl_id, &mut enq);
    drop_field_shadowing_methods(ctx, &ctx.last_segment(id), &fields, &mut methods);
    let identity_qualified = has_qualified_name(ctx, impl_id);
    EntityDef {
        id,
        impl_id,
        fields,
        methods,
        identity_qualified,
    }
}

fn build_leaf_enum(ctx: &Ctx, id: Id) -> LeafEnumDef {
    let e = ctx.item(id).and_then(as_enum).unwrap();
    let mut variants = Vec::new();
    for vid in &e.variants {
        let Some(vit) = ctx.item(*vid) else { continue };
        let Some(v) = as_variant(vit) else { continue };
        let pat = match v.kind {
            VariantKind::Plain => "unit",
            VariantKind::Tuple(_) => "tuple",
            VariantKind::Struct { .. } => "struct",
        };
        variants.push((vit.name.clone().unwrap_or_default(), pat));
    }
    LeafEnumDef { id, variants }
}

// ---------------------------------------------------------------------------
// Name resolution (Python-name disambiguation)
// ---------------------------------------------------------------------------

/// Python names for every emitted class, keyed by rustdoc `Id` for primaries and
/// by `(union_id, native_variant)` for subclasses.
pub struct Names {
    /// Primary py name per `Id` (union / entity / leaf_enum). For unions the
    /// special `Type`/`Value`/`Symbol` are kept verbatim.
    primary: BTreeMap<u32, String>,
    /// Subclass py name per `(union_id, native_variant)`.
    subclass: BTreeMap<(u32, String), String>,
}

impl Names {
    fn union(&self, id: Id) -> &str {
        self.primary.get(&id.0).map(String::as_str).unwrap_or("?")
    }
    fn entity(&self, id: Id) -> &str {
        self.primary.get(&id.0).map(String::as_str).unwrap_or("?")
    }
    /// Every emitted Python class name (primaries + subclasses) — the RESOLVED,
    /// post-disambiguation names the driver intersects with the AST node set to
    /// derive the entity-shadowed nodes.
    pub fn all_python_names(&self) -> BTreeSet<String> {
        self.primary
            .values()
            .chain(self.subclass.values())
            .cloned()
            .collect()
    }
}

pub fn resolve_names(ctx: &Ctx, r: &Reflected) -> Names {
    // 1) Primary names: unions, entities, leaf_enums.
    let mut primary_ids: Vec<Id> = Vec::new();
    for u in &r.unions {
        primary_ids.push(u.id);
    }
    for e in &r.entities {
        primary_ids.push(e.id);
    }
    for l in &r.leaf_enums {
        primary_ids.push(l.id);
    }
    // Count bare last-segment usage among primaries.
    let mut prim_count: BTreeMap<String, usize> = BTreeMap::new();
    for id in &primary_ids {
        *prim_count.entry(ctx.last_segment(*id)).or_default() += 1;
    }
    let mut primary: BTreeMap<u32, String> = BTreeMap::new();
    for id in &primary_ids {
        let bare = ctx.last_segment(*id);
        // Prefix when the bare name collides among emitted classes *or* is a
        // reserved name that shadows/confuses a std/crate type.
        let name = if prim_count.get(&bare).copied().unwrap_or(0) > 1
            || RESERVED_TYPE_NAMES.contains(&bare.as_str())
        {
            format!("{}{}", ctx.module_prefix(*id), bare)
        } else {
            bare
        };
        primary.insert(id.0, name);
    }

    // 2) Subclass names, unique against primaries + each other.
    let mut used: BTreeMap<String, usize> = BTreeMap::new();
    for n in primary.values() {
        *used.entry(n.clone()).or_default() += 1;
    }
    // Tentative subclass names.
    let mut tentatives: Vec<(u32, String, String)> = Vec::new(); // (union_id, variant, tentative)
    for u in &r.unions {
        for v in &u.variants {
            let tentative = match &v.payload {
                VariantPayload::Struct(sid) => ctx.last_segment(*sid),
                _ => v.native.clone(),
            };
            *used.entry(tentative.clone()).or_default() += 1;
            tentatives.push((u.id.0, v.native.clone(), tentative));
        }
    }
    let mut subclass: BTreeMap<(u32, String), String> = BTreeMap::new();
    for (uid, variant, tentative) in tentatives {
        // Prefix when the tentative name collides among emitted classes *or* is a
        // reserved name that shadows/confuses a std/crate type.
        let name = if used.get(&tentative).copied().unwrap_or(0) > 1
            || RESERVED_TYPE_NAMES.contains(&tentative.as_str())
        {
            format!(
                "{}{}",
                primary.get(&uid).cloned().unwrap_or_default(),
                variant
            )
        } else {
            tentative
        };
        subclass.insert((uid, variant), name);
    }

    Names { primary, subclass }
}

// ---------------------------------------------------------------------------
// Emit
// ---------------------------------------------------------------------------

fn render_shape(names: &Names, s: &Shape) -> String {
    match s {
        Shape::Bool => "bool".into(),
        Shape::I128 => "i128".into(),
        Shape::F64 => "f64".into(),
        Shape::Usize => "usize".into(),
        Shape::Str => "str".into(),
        Shape::Node => "node".into(),
        Shape::Span => "span".into(),
        Shape::Unit => "unit".into(),
        // Every closed union is referenced generically by its Python name; the
        // macro has no per-union shorthand vocabulary to keep in sync.
        Shape::Union(id) => format!("union({})", names.union(*id)),
        Shape::Rewrap(uid, variant) => {
            format!("rewrap({}::{})", names.union(*uid), variant)
        }
        Shape::Entity(id) => format!("entity({})", names.entity(*id)),
        Shape::LeafEnum(id) => format!("leaf(crate::sem::{})", names.entity(*id)),
        Shape::LeafAst(name) => format!("leaf(crate::ast::{name})"),
        Shape::AstDef(name) => format!("astdef({name})"),
        Shape::Opt(s) => format!("opt({})", render_shape(names, s)),
        Shape::List(s) => format!("list({})", render_shape(names, s)),
        Shape::Map(k, v) => format!(
            "map({}, {})",
            render_shape(names, k),
            render_shape(names, v)
        ),
        Shape::Tuple(v) => {
            let parts: Vec<String> = v.iter().map(|e| render_shape(names, e)).collect();
            format!("tuple({})", parts.join(", "))
        }
        Shape::Skip(_) => "skip".into(),
    }
}

/// Render an argkind token. Local type references resolve to their (now known)
/// Python class names, exactly as [`render_shape`] does.
fn render_arg(names: &Names, a: &Arg) -> String {
    match a {
        Arg::Analysis => "analysis".into(),
        Arg::Scalar(s) => (*s).into(),
        Arg::Node => "node".into(),
        Arg::Span => "span".into(),
        Arg::AstNode(name) => format!("astnode({name})"),
        Arg::Union(id) => format!("union({})", names.union(*id)),
        Arg::Entity(id) => format!("entity({})", names.entity(*id)),
        Arg::Arc(i) => format!("arc({})", render_arg(names, i)),
        Arg::Opt(i) => format!("opt({})", render_arg(names, i)),
        Arg::List(i) => format!("list({})", render_arg(names, i)),
    }
}

fn render_method(names: &Names, m: &MethodDef) -> String {
    let assoc = if m.assoc { "assoc " } else { "" };
    // `ref` on the return is the peer of `ref` on a param: it records the native's
    // pass form, which the shape itself does not carry.
    let by_ref = if m.ret_ref { "ref " } else { "" };
    let ret = format!("{by_ref}{}", render_shape(names, &m.ret));
    // `throws` is a call-site transform (the native `Result` becomes a raise), not a
    // value conversion, so it sits on the method rather than inside the shape.
    let throws = if m.throws.is_some() { " throws" } else { "" };
    if m.params.is_empty() {
        format!("{assoc}{}{throws} -> {ret},", m.name)
    } else {
        let params: Vec<String> = m
            .params
            .iter()
            .map(|(n, spec)| {
                let kw = render_arg(names, &spec.arg);
                let by_ref = if spec.borrowed { "ref " } else { "" };
                format!("{n}: {by_ref}{kw}")
            })
            .collect();
        format!("{assoc}{}({}){throws} -> {ret},", m.name, params.join(", "))
    }
}

fn log_skips(names: &Names, prefix: &str, fields: &[(String, Shape)], out: &mut Vec<String>) {
    for (fname, sh) in fields {
        if let Shape::Skip(reason) = sh {
            out.push(format!("  skip {prefix}.{fname}: {reason}"));
        } else if let Shape::Map(k, _) = sh {
            let _ = k;
            let _ = render_shape(names, sh);
        }
    }
}

pub fn emit(
    ctx: &Ctx,
    r: &Reflected,
    names: &Names,
    version: &str,
    skips: &mut Vec<String>,
) -> String {
    let mut out = String::new();
    out.push_str(&header(version));
    out.push_str("fpp_python_macros::fpp_sem_bindings! {\n");

    // --- traits whose methods are reflected (brought into scope by the macro) ---
    let traits = reflected_traits(r);
    if !traits.is_empty() {
        out.push_str("    traits {\n");
        for t in &traits {
            out.push_str(&format!("        {t},\n"));
        }
        out.push_str("    }\n\n");
    }

    // --- analysis root ---
    out.push_str(&format!(
        "    analysis native {} {{\n",
        ctx.native_path(r.analysis_id)
    ));
    out.push_str("        fields {\n");
    for (fname, sh) in &r.analysis_fields {
        if let Shape::Skip(reason) = sh {
            skips.push(format!("  skip Analysis.{fname}: {reason}"));
        }
        out.push_str(&format!(
            "            {fname}: {},\n",
            render_shape(names, sh)
        ));
    }
    out.push_str("        }\n");
    if !r.analysis_methods.is_empty() {
        out.push_str("        methods {\n");
        for m in &r.analysis_methods {
            out.push_str(&format!("            {}\n", render_method(names, m)));
        }
        out.push_str("        }\n");
    }
    out.push_str("    }\n\n");

    // --- unions (sorted by py name) ---
    let mut unions: Vec<&UnionDef> = r.unions.iter().collect();
    unions.sort_by_key(|u| names.union(u.id).to_string());
    for u in unions {
        emit_union(ctx, names, u, &mut out, skips);
    }

    // --- payloads (sorted by py name = subclass name) ---
    let mut payloads: Vec<&PayloadDef> = r.payloads.iter().collect();
    payloads.sort_by_key(|p| payload_name(ctx, names, p.id));
    for p in payloads {
        emit_payload(ctx, names, p, &mut out, skips);
    }

    // --- entities (sorted by py name) ---
    let mut entities: Vec<&EntityDef> = r.entities.iter().collect();
    entities.sort_by_key(|e| names.entity(e.id).to_string());
    for e in entities {
        emit_entity(ctx, names, e, &mut out, skips);
    }

    // --- leaf enums (sorted by py name) ---
    let mut leaves: Vec<&LeafEnumDef> = r.leaf_enums.iter().collect();
    leaves.sort_by_key(|l| names.entity(l.id).to_string());
    for l in leaves {
        emit_leaf_enum(ctx, names, l, &mut out);
    }

    out.push_str("}\n");
    out
}

/// The py name of a payload = the subclass name of the variant that owns it.
fn payload_name(ctx: &Ctx, names: &Names, sid: Id) -> String {
    if let Some((uid, variant)) = ctx.payload_index.get(&sid.0)
        && let Some(n) = names.subclass.get(&(uid.0, variant.clone()))
    {
        return n.clone();
    }
    ctx.last_segment(sid)
}

/// Select a union's storage `handle`/`accessor` + `identity`/`repr` directives.
///
/// The three special unions (arc-shared `Type`, by-value `Value`, symbol-keyed
/// `Symbol`) are recognized by their NATIVE path — not the resolved *Python* name.
/// The Python name is a post-disambiguation display string that a name collision
/// can rewrite; keying selection on it would silently downgrade a renamed special
/// union to a plain `clone` (dropping `Symbol`'s value identity, `Type`'s Arc
/// storage, …). The native path is disambiguation-invariant. A rename of the
/// underlying `fpp_analysis` type instead falls through to `clone` here, and
/// [`assert_special_union_handles`] then FAILS LOUD if the union still
/// *structurally* looks arc/symbol-shaped, so the downgrade can never pass
/// silently.
fn union_directives(ctx: &Ctx, u: &UnionDef) -> (String, String, String, String, String, String) {
    // (handle, accessor, extras, identity, repr, ...) — matching the known-good
    // Type/Value/Symbol conventions.
    let (handle, accessor, mut extras) = match ctx.native_path(u.id).as_str() {
        "fpp_analysis::semantics::Type" => {
            ("arc_type", "ty", " include_base custom_build".to_string())
        }
        "fpp_analysis::semantics::Value" => ("value", "val", String::new()),
        "fpp_analysis::semantics::Symbol" => ("symbol", "sym", String::new()),
        _ => ("clone", "native", String::new()),
    };
    // `loc_from_node` for clone unions whose native carries a node (SymbolInterface).
    if handle == "clone" && has_no_arg_node(ctx, u.id) {
        extras.push_str(" loc_from_node");
    }
    // Identity/repr directives carry the `fpp_analysis` method identifiers they
    // call as explicit payloads, so a rename regenerates a still-correct call site
    // in the macro instead of a compile break or silent downgrade.
    let identity = match handle {
        "arc_type" => " identity identical(identical, def_node_id)".to_string(),
        "symbol" => " identity node".to_string(),
        _ => String::new(),
    };
    let repr = match handle {
        "arc_type" => String::new(),
        "value" => " repr variant".to_string(),
        "symbol" => " repr variant_qualified(get_qualified_name)".to_string(),
        _ => {
            if has_unqualified_name(ctx, u.id) {
                " repr variant_unqualified(get_unqualified_name)".to_string()
            } else {
                " repr variant".to_string()
            }
        }
    };
    (
        handle.to_string(),
        accessor.to_string(),
        extras,
        identity,
        repr,
        String::new(),
    )
}

/// The core `fpp_analysis` semantic hierarchies that reflection from the root
/// [`ROOT_TYPE_NAME`] struct must always reach. Losing one signals the reflection
/// root or a public field path changed; [`assert_core_unions_present`] turns that
/// into a loud abort instead of a silently-shrunken binding surface.
const CORE_UNION_NATIVES: &[&str] = &[
    "fpp_analysis::semantics::Type",
    "fpp_analysis::semantics::Value",
    "fpp_analysis::semantics::Symbol",
];

/// FAIL LOUD if reflection failed to reach one of the [`CORE_UNION_NATIVES`].
pub fn assert_core_unions_present(ctx: &Ctx, r: &Reflected) {
    for native in CORE_UNION_NATIVES {
        assert!(
            r.unions.iter().any(|u| ctx.native_path(u.id) == *native),
            "core union `{native}` was not reflected from `{ROOT_TYPE_NAME}` — the \
             reflection root or a public field path changed"
        );
    }
}

/// FAIL LOUD if the two `fpp_core` types the [`classify`] `fpp_core` arm keys on by
/// exact name (`Node`/`Span`) no longer resolve. The arm matches by name, so a
/// rename in `fpp_core` would silently downgrade every referencing field to `skip`;
/// this aborts with a clear message instead.
fn assert_fpp_core_types(ctx: &Ctx) {
    for name in ["Node", "Span"] {
        let found = ctx.krate.paths.values().any(|s| {
            s.path.last().map(String::as_str) == Some(name)
                && ctx
                    .krate
                    .external_crates
                    .get(&s.crate_id)
                    .map(|c| c.name.as_str())
                    == Some("fpp_core")
        });
        assert!(
            found,
            "fpp_core::{name} not found in rustdoc paths — the `fpp_core` classification \
             arm keys on this exact name; update `classify` if fpp_core renamed it"
        );
    }
}

/// FAIL LOUD if the [`DEF_MODULE_STUB`] bridge type no longer exists. The
/// reflection maps it to `astdef(`[`DEF_MODULE_AST`]`)` by name, so its
/// disappearance would silently drop the module bridge.
fn assert_def_module_stub(ctx: &Ctx) {
    let found = ctx.krate.index.values().any(|it| {
        it.name.as_deref() == Some(DEF_MODULE_STUB)
            && matches!(it.inner, ItemEnum::Struct(_))
            && ctx.is_local(it.id)
    });
    assert!(
        found,
        "`{DEF_MODULE_STUB}` struct not found in fpp_analysis — the {DEF_MODULE_STUB}→\
         {DEF_MODULE_AST} bridge keys on this name"
    );
}

/// FAIL LOUD if a union that *structurally* requires a special handle would be
/// emitted as a plain `clone` (the silent-downgrade hazard behind [finding #1]).
/// Two independent structural signals gate the check:
///   * arc-shared — the enum is handed across the `fpp_analysis` API behind `Arc<…>`
///     (only `Type` today); such a union MUST use the `arc_type` handle.
///   * symbol-keyed — the enum implements `SymbolInterface` AND keys one of the
///     root `Analysis` struct's own map fields (only `Symbol` today; the nested
///     `StateMachineSymbol` also implements the trait but never keys an `Analysis`
///     field, so the map-key refinement is what isolates the identity-bearing
///     case); such a union MUST keep the `symbol` handle + an `identity` directive.
///
/// A future `fpp_analysis` rename that slips a special union through the native-path
/// match in [`union_directives`] trips one of these and aborts the generator with a
/// clear message, instead of silently shipping a broken binding.
pub fn assert_special_union_handles(ctx: &Ctx, r: &Reflected) {
    let arc_ids = arc_api_enum_ids(ctx);
    assert_arc_signal_live(ctx, r, &arc_ids);
    let mut analysis_key_unions: BTreeSet<u32> = BTreeSet::new();
    let mut analysis_key_entities: BTreeSet<u32> = BTreeSet::new();
    for (_, sh) in &r.analysis_fields {
        collect_map_keys(sh, &mut analysis_key_unions, &mut analysis_key_entities);
    }
    for u in &r.unions {
        let (handle, .., identity, _repr, _) = union_directives(ctx, u);
        let native = ctx.native_path(u.id);
        if arc_ids.contains(&u.id.0) && handle != "arc_type" {
            panic!(
                "union `{native}` is handed across the fpp_analysis API behind Arc<…> but \
                 was classified as `{handle}` — add it to the special-union mapping in \
                 `union_directives` (arc_type handle)"
            );
        }
        if impls_symbol_interface(ctx, u.id)
            && analysis_key_unions.contains(&u.id.0)
            && (handle != "symbol" || identity.is_empty())
        {
            panic!(
                "union `{native}` implements SymbolInterface and keys a root Analysis \
                 map but was classified as `{handle}`{} — add it to the special-union \
                 mapping in `union_directives` (symbol handle + `identity node`)",
                if identity.is_empty() {
                    " with no identity"
                } else {
                    ""
                }
            );
        }
    }
}

/// FAIL LOUD if the arc-shared signal in [`arc_api_enum_ids`] no longer fires for
/// the one union that is known to need the `arc_type` handle. That signal is
/// narrowed to public fn/alias positions, so an `fpp_analysis` refactor that moved
/// `Arc<Type>` entirely out of them (onto a trait, or behind private helpers) would
/// leave the arc guard testing nothing — and the next union to need `arc_type` would
/// then be downgraded to `clone` silently, which is exactly what the guard exists to
/// prevent. `Type`'s presence in the reflected model is `assert_core_unions_present`'s
/// job; this asserts only that the *signal* still sees it.
fn assert_arc_signal_live(ctx: &Ctx, r: &Reflected, arc_ids: &BTreeSet<u32>) {
    const ARC_UNION_NATIVE: &str = "fpp_analysis::semantics::Type";
    let ty = r
        .unions
        .iter()
        .find(|u| ctx.native_path(u.id) == ARC_UNION_NATIVE);
    if let Some(ty) = ty {
        assert!(
            arc_ids.contains(&ty.id.0),
            "the arc-shared signal no longer detects `{ARC_UNION_NATIVE}` — \
             `Arc<{ARC_UNION_NATIVE}>` has left every public fn signature / type alias \
             in fpp_analysis, so `arc_api_enum_ids` is now blind and the arc arm of \
             `assert_special_union_handles` guards nothing; widen the signal to \
             wherever the Arc moved"
        );
    }
}

/// Local enum `Id`s that `fpp_analysis` hands across its API behind an `Arc<…>`
/// (fn signatures and type aliases) — the structural signal for the `arc_type`
/// handle: the binding has to hold the same `Arc` the API gave it, or the shared
/// identity the handle exposes (`Type.identical`) is lost.
///
/// Deliberately NOT every `Arc<…>` position in the crate. Two kinds of `Arc` are
/// *not* API identity, and counting them would abort the generator on a union that
/// is classified correctly:
///   * STRUCT FIELDS — a field can be internal structural sharing
///     (`AnonArrayValue::elements: Vec<Arc<Value>>` stores one allocation for the
///     repeated elements of an array default) while every `Value` the API hands out
///     is still by value. The reflection peels those `Arc`s away.
///   * PRIVATE fns — the rustdoc JSON is built with `document_private_items`, so
///     the index carries crate-internal helpers (`AnonArrayValue::map_elements ->
///     Vec<Arc<Value>>`) whose signatures are not API the binding can ever see.
///
/// Restricting to `Visibility::Public` also drops trait methods (rustdoc gives
/// those `Default` visibility, indistinguishable from private here), so the signal
/// would go blind if `Arc<Type>` ever appeared *only* on a trait. [`Type`'s own
/// arc-ness is asserted by the caller](assert_arc_signal_live) to catch that.
fn arc_api_enum_ids(ctx: &Ctx) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for it in ctx.krate.index.values() {
        if !is_public(it) {
            continue;
        }
        match &it.inner {
            ItemEnum::TypeAlias(a) => find_arc_wrapped(ctx, &a.type_, &mut out),
            ItemEnum::Function(f) => {
                for (_, t) in &f.sig.inputs {
                    find_arc_wrapped(ctx, t, &mut out);
                }
                if let Some(o) = &f.sig.output {
                    find_arc_wrapped(ctx, o, &mut out);
                }
            }
            _ => {}
        }
    }
    out
}

fn find_arc_wrapped(ctx: &Ctx, ty: &Type, out: &mut BTreeSet<u32>) {
    match ty {
        Type::ResolvedPath(p) => {
            let last = p.path.rsplit("::").next().unwrap_or(&p.path);
            let args = path_type_args(&p.args);
            if last == "Arc"
                && let Some(inner) = args.first()
                && let Type::ResolvedPath(ip) = peel_wrappers(inner)
                && ctx.is_local(ip.id)
                && matches!(ctx.kind(ip.id), Some(ItemKind::Enum))
            {
                out.insert(ip.id.0);
            }
            for a in args {
                find_arc_wrapped(ctx, a, out);
            }
        }
        Type::BorrowedRef { type_, .. } => find_arc_wrapped(ctx, type_, out),
        Type::Tuple(elems) => {
            for e in elems {
                find_arc_wrapped(ctx, e, out);
            }
        }
        Type::Slice(t) => find_arc_wrapped(ctx, t, out),
        Type::Array { type_, .. } => find_arc_wrapped(ctx, type_, out),
        _ => {}
    }
}

/// Whether a local type implements `Clone` (derived or hand-written). The
/// clone-entity handle stores the native by value and clones it, so an entity
/// struct without `Clone` cannot be emitted.
fn impls_clone(ctx: &Ctx, id: Id) -> bool {
    impls_std_marker(ctx, id, "Clone")
}

/// Whether a local type implements the `Debug` trait (derived or hand-written).
/// A `Result<_, E>` return is only marshallable when `E` can be rendered into the
/// raised Python exception, which the generated code does with `{:?}`.
fn impls_debug(ctx: &Ctx, id: Id) -> bool {
    impls_std_marker(ctx, id, "Debug")
}

/// Whether a local type has a trait impl whose trait's last path segment is `name`.
fn impls_std_marker(ctx: &Ctx, id: Id, name: &str) -> bool {
    impls_on(ctx, id).iter().any(|imp| {
        imp.trait_.as_ref().is_some_and(|tr| {
            tr.path.rsplit("::").next().unwrap_or(&tr.path) == name
                // These markers live in `core`; requiring a foreign crate stops a
                // same-named `fpp_analysis` trait from satisfying the check.
                && !ctx.is_local(tr.id)
        })
    })
}

/// Whether a local type implements the `fpp_analysis` `SymbolInterface` trait.
fn impls_symbol_interface(ctx: &Ctx, id: Id) -> bool {
    impls_on(ctx, id).iter().any(|imp| {
        imp.trait_.as_ref().is_some_and(|tr| {
            tr.path.rsplit("::").next().unwrap_or(&tr.path) == "SymbolInterface"
                && ctx.crate_name(tr.id).as_deref() == Some("fpp_analysis")
        })
    })
}

/// Walk `s`, collecting the union / entity `Id`s that appear in any map KEY
/// position (recursing through nested maps/opt/list/tuple).
fn collect_map_keys(s: &Shape, ku: &mut BTreeSet<u32>, ke: &mut BTreeSet<u32>) {
    match s {
        Shape::Map(k, v) => {
            collect_key_ids(k, ku, ke);
            collect_map_keys(k, ku, ke);
            collect_map_keys(v, ku, ke);
        }
        Shape::Opt(i) | Shape::List(i) => collect_map_keys(i, ku, ke),
        Shape::Tuple(v) => v.iter().for_each(|e| collect_map_keys(e, ku, ke)),
        _ => {}
    }
}

/// Collect every union / entity `Id` referenced anywhere inside `s` (a key shape).
fn collect_key_ids(s: &Shape, ku: &mut BTreeSet<u32>, ke: &mut BTreeSet<u32>) {
    match s {
        Shape::Union(id) => {
            ku.insert(id.0);
        }
        Shape::Entity(id) => {
            ke.insert(id.0);
        }
        Shape::Opt(i) | Shape::List(i) => collect_key_ids(i, ku, ke),
        Shape::Tuple(v) => v.iter().for_each(|e| collect_key_ids(e, ku, ke)),
        Shape::Map(k, v) => {
            collect_key_ids(k, ku, ke);
            collect_key_ids(v, ku, ke);
        }
        _ => {}
    }
}

/// Every reflected method in the model, in emission order.
fn all_methods(r: &Reflected) -> impl Iterator<Item = &MethodDef> {
    r.analysis_methods
        .iter()
        .chain(r.unions.iter().flat_map(|u| &u.methods))
        .chain(r.payloads.iter().flat_map(|p| &p.methods))
        .chain(r.entities.iter().flat_map(|e| &e.methods))
}

/// The `fpp_analysis` traits the reflected methods come from. The generated code
/// brings each into scope so a trait method's call site resolves; without this a
/// reflected trait method would emit `self.<recv>.<m>(…)` for a method not visible
/// there. Deduped and sorted, so the emitted section is stable.
fn reflected_traits(r: &Reflected) -> BTreeSet<String> {
    all_methods(r)
        .filter_map(|m| m.trait_path.clone())
        .collect()
}

/// Every conversion `Shape` reachable in the reflected model (analysis, unions,
/// payloads, entities — fields, variant payloads, and method returns).
fn all_shapes(r: &Reflected) -> Vec<&Shape> {
    let mut v: Vec<&Shape> = Vec::new();
    for (_, s) in &r.analysis_fields {
        v.push(s);
    }
    for m in &r.analysis_methods {
        v.push(&m.ret);
    }
    for u in &r.unions {
        for var in &u.variants {
            match &var.payload {
                VariantPayload::Value(s) | VariantPayload::Newtype(s) => v.push(s),
                VariantPayload::StructVariant(fs) => fs.iter().for_each(|(_, s)| v.push(s)),
                _ => {}
            }
        }
        for m in &u.methods {
            v.push(&m.ret);
        }
    }
    for p in &r.payloads {
        for (_, s) in &p.fields {
            v.push(s);
        }
        for m in &p.methods {
            v.push(&m.ret);
        }
    }
    for e in &r.entities {
        for (_, s) in &e.fields {
            v.push(s);
        }
        for m in &e.methods {
            v.push(&m.ret);
        }
    }
    v
}

/// FAIL LOUD (as logged `WARN` lines) for every union / entity used as a `map(...)`
/// KEY that carries NO identity directive: without `__eq__`/`__hash__` its Python
/// wrapper falls back to object identity, so a dict lookup with a freshly-built key
/// silently misses. Logged rather than aborting because the current read-only maps
/// legitimately expose such keys for iteration only, and a hard abort would block
/// regeneration; the diagnostic still surfaces every occurrence on each run.
pub fn warn_identityless_map_keys(
    ctx: &Ctx,
    r: &Reflected,
    names: &Names,
    skips: &mut Vec<String>,
) {
    let mut union_has_identity: BTreeMap<u32, bool> = BTreeMap::new();
    for u in &r.unions {
        let (_, _, _, identity, _, _) = union_directives(ctx, u);
        union_has_identity.insert(u.id.0, !identity.is_empty());
    }
    let entity_has_identity: BTreeMap<u32, bool> = r
        .entities
        .iter()
        .map(|e| (e.id.0, e.identity_qualified))
        .collect();

    let mut key_unions: BTreeSet<u32> = BTreeSet::new();
    let mut key_entities: BTreeSet<u32> = BTreeSet::new();
    for sh in all_shapes(r) {
        collect_map_keys(sh, &mut key_unions, &mut key_entities);
    }

    for uid in key_unions {
        if !union_has_identity.get(&uid).copied().unwrap_or(false) {
            skips.push(format!(
                "  WARN union {} keys a map but has no identity directive \
                 (dict lookups by a rebuilt key will miss)",
                names.union(Id(uid))
            ));
        }
    }
    for eid in key_entities {
        if !entity_has_identity.get(&eid).copied().unwrap_or(false) {
            skips.push(format!(
                "  WARN entity {} keys a map but has no identity directive \
                 (dict lookups by a rebuilt key will miss)",
                names.entity(Id(eid))
            ));
        }
    }
}

fn has_no_arg_node(ctx: &Ctx, id: Id) -> bool {
    has_no_arg_method_returning_node(ctx, id, "node")
}
fn has_unqualified_name(ctx: &Ctx, id: Id) -> bool {
    method_names(ctx, id).contains(&"get_unqualified_name".to_string())
}

fn method_names(ctx: &Ctx, id: Id) -> Vec<String> {
    let mut out = Vec::new();
    for imp in impls_on(ctx, id) {
        for mid in &imp.items {
            if let Some(mi) = ctx.item(*mid)
                && matches!(mi.inner, ItemEnum::Function(_))
                && let Some(n) = &mi.name
            {
                out.push(n.clone());
            }
        }
    }
    out
}

fn has_no_arg_method_returning_node(ctx: &Ctx, id: Id, name: &str) -> bool {
    for imp in impls_on(ctx, id) {
        for mid in &imp.items {
            let Some(mi) = ctx.item(*mid) else { continue };
            if mi.name.as_deref() != Some(name) {
                continue;
            }
            let ItemEnum::Function(f) = &mi.inner else {
                continue;
            };
            if f.sig.inputs.len() == 1
                && f.sig.inputs[0].0 == "self"
                && let Some(Type::ResolvedPath(p)) = &f.sig.output
            {
                let last = p.path.rsplit("::").next().unwrap_or(&p.path);
                if last == "Node" && ctx.crate_name(p.id).as_deref() == Some("fpp_core") {
                    return true;
                }
            }
        }
    }
    false
}

fn emit_union(ctx: &Ctx, names: &Names, u: &UnionDef, out: &mut String, skips: &mut Vec<String>) {
    let py = names.union(u.id).to_string();
    let (handle, accessor, extras, identity, repr, _) = union_directives(ctx, u);
    out.push_str(&format!(
        "    union {py} native {} handle {handle} alias \"{py}\" accessor {accessor}{extras}{identity}{repr} {{\n",
        ctx.native_path(u.id)
    ));
    out.push_str("        variants {\n");
    for v in &u.variants {
        let sub = names
            .subclass
            .get(&(u.id.0, v.native.clone()))
            .cloned()
            .unwrap_or_else(|| v.native.clone());
        let kind = match &v.payload {
            VariantPayload::Unit => "unit".to_string(),
            VariantPayload::Value(sh) => render_shape(names, sh),
            VariantPayload::Newtype(sh) => format!("newtype({})", render_shape(names, sh)),
            VariantPayload::Struct(_) => "payload".to_string(),
            VariantPayload::StructVariant(fs) => {
                let parts: Vec<String> = fs
                    .iter()
                    .map(|(n, sh)| {
                        if let Shape::Skip(reason) = sh {
                            skips.push(format!("  skip {py}::{}.{n}: {reason}", v.native));
                        }
                        format!("{n}: {}", render_shape(names, sh))
                    })
                    .collect();
                format!("struct {{ {} }}", parts.join(", "))
            }
            VariantPayload::Other => {
                skips.push(format!(
                    "  skip {py}::{} (multi-field tuple variant unsupported)",
                    v.native
                ));
                "unit".to_string()
            }
        };
        out.push_str(&format!("            {} => {sub} : {kind},\n", v.native));
    }
    out.push_str("        }\n");
    emit_methods(names, &u.methods, out);
    out.push_str("    }\n\n");
}

fn emit_payload(
    ctx: &Ctx,
    names: &Names,
    p: &PayloadDef,
    out: &mut String,
    skips: &mut Vec<String>,
) {
    let name = payload_name(ctx, names, p.id);
    out.push_str(&format!(
        "    payload {name} native {} {{\n",
        ctx.native_path(p.id)
    ));
    out.push_str("        fields {\n");
    for (fname, sh) in &p.fields {
        if let Shape::Skip(reason) = sh {
            skips.push(format!("  skip {name}.{fname}: {reason}"));
        }
        out.push_str(&format!(
            "            {fname}: {},\n",
            render_shape(names, sh)
        ));
    }
    out.push_str("        }\n");
    emit_methods(names, &p.methods, out);
    out.push_str("    }\n\n");
}

fn emit_entity(ctx: &Ctx, names: &Names, e: &EntityDef, out: &mut String, skips: &mut Vec<String>) {
    let name = names.entity(e.id).to_string();
    let identity = if e.identity_qualified {
        " identity qualified_name(qualified_name)"
    } else {
        ""
    };
    out.push_str(&format!(
        "    entity {name} native {}{identity} {{\n",
        ctx.native_path(e.id)
    ));
    // Opaque = empty entity (no fields / methods).
    if e.fields.is_empty() && e.methods.is_empty() {
        out.push_str("    }\n\n");
        return;
    }
    if !e.fields.is_empty() {
        out.push_str("        fields {\n");
        for (fname, sh) in &e.fields {
            if let Shape::Skip(reason) = sh {
                skips.push(format!("  skip {name}.{fname}: {reason}"));
            }
            out.push_str(&format!(
                "            {fname}: {},\n",
                render_shape(names, sh)
            ));
        }
        out.push_str("        }\n");
    }
    emit_methods(names, &e.methods, out);
    out.push_str("    }\n\n");
}

fn emit_methods(names: &Names, methods: &[MethodDef], out: &mut String) {
    if methods.is_empty() {
        return;
    }
    out.push_str("        methods {\n");
    for m in methods {
        out.push_str(&format!("            {}\n", render_method(names, m)));
    }
    out.push_str("        }\n");
}

fn emit_leaf_enum(ctx: &Ctx, names: &Names, l: &LeafEnumDef, out: &mut String) {
    let name = names.entity(l.id);
    out.push_str(&format!(
        "    leaf_enum {name} native {} {{\n",
        ctx.native_path(l.id)
    ));
    for (variant, pat) in &l.variants {
        out.push_str(&format!("        {variant}: {pat},\n"));
    }
    out.push_str("    }\n\n");
}

fn header(version: &str) -> String {
    format!(
        "// @generated by bindgen from fpp_analysis v{version} — do not edit\n\
         //! Declarative mirror of the `fpp_analysis` semantic layer, expanded by the\n\
         //! `fpp_python_macros::fpp_sem_bindings!` proc macro into the read-only PyO3\n\
         //! wrappers for the semantic data structures. GENERATED by `bindgen`\n\
         //! (rustdoc-JSON reflection of `fpp_analysis`, rooted at `Analysis`) — do not\n\
         //! edit by hand; regenerate with\n\
         //! `cargo run -p fpp_python_bindgen`.\n\
         #![allow(dead_code, unused_variables, clippy::all)]\n\n"
    )
}

pub fn write_out(path: &std::path::Path, text: &str) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(path, text).unwrap();
}

// ---------------------------------------------------------------------------
// Reflection context construction + the shadow-normalize pass
// ---------------------------------------------------------------------------

/// Build the reflection context over a parsed rustdoc [`Crate`] + the shared AST
/// classification, then fail loud if the by-name bridges (`fpp_core::Node`/`Span`,
/// `DefModuleStub`) no longer resolve — before reflection silently routes around
/// them.
pub fn prepare<'a>(krate: &'a Crate, ast: AstClass) -> Ctx<'a> {
    let best_path = build_best_paths(krate);
    let mut ctx = Ctx {
        krate,
        best_path,
        payload_index: BTreeMap::new(),
        newtype_payloads: BTreeSet::new(),
        alias_struct: BTreeMap::new(),
        analysis_id: None,
        ast,
        used_ast_leaves: BTreeSet::new(),
        skips: Vec::new(),
    };
    build_payload_index(&mut ctx);
    build_alias_index(&mut ctx);
    ctx.analysis_id = find_analysis(&ctx);
    assert_fpp_core_types(&ctx);
    assert_def_module_stub(&ctx);
    ctx
}

/// Phase E: rewrite `astdef(x)` → `skip` for every shadowed AST node `x` — its
/// Python name is owned by a semantic class, so the node has no stub and is
/// opaque as a child. Walks the reflected fields + variant payloads; method
/// returns carrying an `astdef` were already dropped by Rule R2, so none survive
/// there. Method *parameters* are also walked: an `astnode(x)` param would name the
/// unstubbed wrapper class in the `.pyi`, so such a method is dropped whole (a
/// parameter, unlike a field, cannot degrade to `skip`). Returns one log line per
/// rewrite.
pub fn apply_shadow(r: &mut Reflected, names: &Names, shadowed: &BTreeSet<String>) -> Vec<String> {
    let mut log = Vec::new();
    for (fname, sh) in &mut r.analysis_fields {
        shadow_shape(sh, shadowed, &format!("Analysis.{fname}"), &mut log);
    }
    drop_shadowed_param_methods(&mut r.analysis_methods, "Analysis", shadowed, &mut log);
    for u in &mut r.unions {
        for v in &mut u.variants {
            let label = v.native.clone();
            match &mut v.payload {
                VariantPayload::Value(sh) | VariantPayload::Newtype(sh) => {
                    shadow_shape(sh, shadowed, &label, &mut log);
                }
                VariantPayload::StructVariant(fs) => {
                    for (n, sh) in fs.iter_mut() {
                        shadow_shape(sh, shadowed, &format!("{label}.{n}"), &mut log);
                    }
                }
                _ => {}
            }
        }
        let owner = names.union(u.id).to_string();
        drop_shadowed_param_methods(&mut u.methods, &owner, shadowed, &mut log);
    }
    for p in &mut r.payloads {
        for (fname, sh) in &mut p.fields {
            shadow_shape(sh, shadowed, fname, &mut log);
        }
        drop_shadowed_param_methods(&mut p.methods, "payload", shadowed, &mut log);
    }
    for e in &mut r.entities {
        for (fname, sh) in &mut e.fields {
            shadow_shape(sh, shadowed, fname, &mut log);
        }
        let owner = names.entity(e.id).to_string();
        drop_shadowed_param_methods(&mut e.methods, &owner, shadowed, &mut log);
    }
    log
}

/// Drop every method taking an `astnode(x)` parameter for a shadowed AST node `x`.
/// The wrapper class exists at runtime but carries no stub, so naming it in a
/// parameter position would emit a `.pyi` referencing an undeclared class.
fn drop_shadowed_param_methods(
    methods: &mut Vec<MethodDef>,
    owner: &str,
    shadowed: &BTreeSet<String>,
    log: &mut Vec<String>,
) {
    let mut dropped: Vec<(String, String)> = Vec::new();
    methods.retain(|m| {
        match m
            .params
            .iter()
            .find_map(|(_, spec)| shadowed_astnode(&spec.arg, shadowed))
        {
            Some(name) => {
                dropped.push((m.name.clone(), name));
                false
            }
            None => true,
        }
    });
    for (m, name) in dropped {
        log.push(format!(
            "  shadow {owner}.{m}(): astnode({name}) param -> method dropped (name owned by a \
             sem class, so the node wrapper has no stub)"
        ));
    }
}

/// The first shadowed AST-node name referenced anywhere inside an argkind.
fn shadowed_astnode(a: &Arg, shadowed: &BTreeSet<String>) -> Option<String> {
    match a {
        Arg::AstNode(name) if shadowed.contains(name) => Some(name.clone()),
        Arg::Arc(i) | Arg::Opt(i) | Arg::List(i) => shadowed_astnode(i, shadowed),
        _ => None,
    }
}

fn shadow_shape(s: &mut Shape, shadowed: &BTreeSet<String>, label: &str, log: &mut Vec<String>) {
    match s {
        Shape::AstDef(name) if shadowed.contains(name) => {
            log.push(format!(
                "  shadow {label}: astdef({name}) -> skip (name owned by a sem class)"
            ));
            *s = Shape::Skip(format!(
                "{name}: shadowed AST node (name owned by a sem class)"
            ));
        }
        Shape::Opt(i) | Shape::List(i) => shadow_shape(i, shadowed, label, log),
        Shape::Map(k, v) => {
            shadow_shape(k, shadowed, label, log);
            shadow_shape(v, shadowed, label, log);
        }
        Shape::Tuple(v) => {
            for e in v {
                shadow_shape(e, shadowed, label, log);
            }
        }
        _ => {}
    }
}
