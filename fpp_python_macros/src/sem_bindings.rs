//! The `fpp_sem_bindings!` function-like macro: expands a declarative mirror of
//! the `fpp_analysis` semantic layer (emitted by the `bindgen` binary into
//! `fpp_python/src/sem/defs.rs`) into the read-only PyO3 wrappers for the
//! semantic data structures — the `Symbol`/`Type`/`Value` closed-union
//! hierarchies, the entity structs, their union `*Ref` newtypes + Python
//! aliases, and the leaf-enum mirrors.
//!
//! This is the semantic-layer analog of `fpp_ast_bindings!` (see
//! [`crate::ast_bindings`]). Unlike the AST — a faithful 1:1 mirror of uniformly
//! `#[ast]`-annotated nodes — the semantic types are plain hand-written
//! structs/enums, so the DSL carries an explicit per-type *handle* (how the
//! wrapper stores its native value) and an explicit *method* list, and every
//! field/method records its resolved *shape* (how the native value converts to a
//! Python object). The macro is a dumb emitter over those shapes; all
//! classification happens in the `bindgen` binary. The macro never reads
//! `fpp_analysis` source — only its DSL tokens.
//!
//! # Grammar
//!
//! ```text
//! union <PyName> native <path> handle <arc_type|value|symbol|clone> alias "<Alias>"
//!       accessor <ident> [include_base] [custom_build] [loc_from_node]
//!       [identity <node | identical[(<eq_fn>, <node_id_method>)]>]
//!       [repr <variant | variant_qualified[(<method>)] | variant_unqualified[(<method>)]>] {
//!     variants { <NativeVariant> => <Subclass> : <payloadkind>, … }
//!     methods  { [assoc] <name> [(analysis)] -> <shape>, … }
//! }
//! payload <PyName> native <path> {
//!     fields  { <name>: <shape>, … }
//!     methods { … }
//! }
//! entity <PyName> native <path> [ field <ident> ] {
//!     extras  { <name>: <shape>, … }   // clone-handle only: build-time scalars
//!     fields  { <name>: <shape>, … }
//!     methods { … }
//! }
//! leaf_enum <PyName> native <path> {
//!     <NativeVariant>: <unit|tuple|struct>, …   // fieldless Python-enum mirror
//! }
//! ```
//!
//! `loc_from_node` emits a base `loc` getter resolving the location from the
//! native's node id. `identity` emits `__eq__`/`__hash__` (`node`: native `==` +
//! hash by node id; `identical`: value equality + node-id hash via the carried
//! `(eq_fn, node_id_method)` idents). `repr` emits `__repr__`
//! (`<Alias Variant [ 'qualified'|'unqualified' name ]>`), resolving the name via
//! the carried method ident. A `leaf_enum` emits a `#[pyclass(eq, eq_int)]` mirror
//! + `From<&native>`.
//!
//! An `entity` is a standalone `#[pyclass(frozen)]` (not a union subclass). It
//! stores a native `Clone` (the default field `native`, or `field <ident>`); its
//! field/method getters project that stored handle.
//!
//! `payloadkind` is `unit` (no data), `payload` (fields from a matching `payload`
//! decl), or a bare `<shape>` (a single-value variant → one `value` getter).
//! `shape` is the conversion vocabulary: `bool i128 f64 usize str node span
//! skip`, `leaf(<path>)`, `astdef(<Ident>)`, `rewrap(<Union>::<Variant>)`,
//! `opt(<shape>)`, `list(<shape>)`, `dict(<shape>)` (string-keyed `dict[str, V]`),
//! `map(<key_shape>, <value_shape>)` (a real `dict[K, V]`), `tuple(<shape>, …)`,
//! `union(<Name>)` (any closed union by its Python name), `entity(<Name>)`.
//!
//! # Additional sections / directives
//!
//! ```text
//! analysis native <path> {                 // the `Analysis` root wrapper
//!     fields  { <name>: <shape>, … }        // read `self.data.analysis.<name>`
//!     methods { <name>(<params>) -> <shape>, … }  // call `self.data.analysis.<name>(…)`
//! }
//! ```
//!
//! A `methods` entry's parameter list is comma-separated `<name>: <argkind>`
//! pairs (plus the legacy bare `analysis` form). `argkind` ∈ `analysis` (the
//! injected `&self.data.analysis`, not a Python param), `symbol` / `symbol_owned`
//! (a Python `Symbol` wrapper feeding a borrowed `&Symbol` / an owned `Symbol`
//! param), and the scalars `i128`/`bool`/`usize`/`str` (borrowed `&str`) /
//! `string` (owned `String`). A method with ≥1 real (non-`analysis`) param is
//! emitted as a callable method; otherwise it is a `#[getter]` property.
//!
//! An `entity` may carry an optional
//! `identity <node | qualified_name[(<method>)] | raw_handle>` directive (peer of
//! the handle/field directives), emitting `__eq__`/`__hash__` in the clone-entity
//! `#[pymethods]` block. Every clone-entity also gets a default `__repr__`
//! (`<PyName>`).

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Path, Token, braced, parenthesized};

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// Per-union metadata a `UnionRef`/`RewrapRef` shape needs to emit a `*Ref`
/// builder call: the native enum path and the storage handle. Built in [`expand`]
/// from the declared `union`s — there is no hardcoded `Type`/`Value`/`Symbol`
/// vocabulary; a union is just its Python name keying this map.
struct UnionInfo {
    /// The native `fpp_analysis` enum path this union mirrors.
    native: Path,
    /// How the native value is stored / handed to the `*_ref` builder.
    handle: Handle,
    /// Native variant name → its concrete Python subclass ident. Lets a
    /// `rewrap(<Union>::<Variant>)` shape — whose native value is statically the
    /// bare payload struct of exactly this variant — refine to the concrete
    /// `Py<Subclass>` instead of the broad union alias.
    subclasses: std::collections::BTreeMap<String, Ident>,
}

/// Union Python name → its [`UnionInfo`]. The `*Ref` newtype and `*_ref`/`build_*`
/// fn names are derived from the Python name (see [`union_ref_ty`]/[`union_ref_fn`]),
/// so only the native path + handle + variant→subclass map are stored.
type UnionReg = std::collections::BTreeMap<String, UnionInfo>;

/// The `crate::sem::<Name>Ref` return newtype for a union's Python name.
fn union_ref_ty(name: &str) -> TokenStream {
    let id = format_ident!("{}Ref", name);
    quote!(crate::sem::#id)
}

/// The `crate::sem::<snake>_ref` builder fn for a union's Python name.
fn union_ref_fn(name: &str) -> TokenStream {
    let id = format_ident!("{}_ref", snake(name));
    quote!(crate::sem::#id)
}

/// The `crate::sem::build_<snake>` dispatching builder fn for a union's Python
/// name (returns `Py<Base>`, boxed as the concrete subclass at runtime).
fn union_build_fn(name: &str) -> TokenStream {
    let id = format_ident!("build_{}", snake(name));
    quote!(crate::sem::#id)
}

/// The concrete subclass ident for a `rewrap(<Union>::<Variant>)`: the Python
/// subclass the union's `Variant` boxes to. Panics on an unknown union/variant
/// (a macro-authoring error — the DSL is machine-generated from the unions here).
fn rewrap_subclass<'a>(reg: &'a UnionReg, name: &str, variant: &Ident) -> &'a Ident {
    let info = reg
        .get(name)
        .unwrap_or_else(|| panic!("unregistered union `{name}`"));
    info.subclasses
        .get(&variant.to_string())
        .unwrap_or_else(|| panic!("union `{name}` has no variant `{variant}`"))
}

/// A field/method conversion shape: a native value → a Python object.
enum Shape {
    Bool,
    I128,
    F64,
    Usize,
    Str,
    Node,
    /// A lazy source-span handle → `Py<crate::ir_core::Span>`, resolved on demand.
    Span,
    /// A registered closed union → its `*Ref` wrapper. The `String` keys the
    /// union registry (`Type`/`Value`/`Symbol`/`PortInstance`); see
    /// [`registered_union`].
    UnionRef(String),
    /// A bare payload struct rewrapped into a registered union's variant before
    /// conversion, e.g. `rewrap(Type::AnonArray)` on a field of type
    /// `AnonArrayType`. The `String` keys the union registry.
    RewrapRef(String, Ident),
    Leaf(Path),
    AstDef(Ident),
    Opt(Box<Shape>),
    List(Box<Shape>),
    /// A `String`-keyed map → a Python `dict[str, V]`.
    Dict(Box<Shape>),
    /// An arbitrary native map → a real Python `dict[K, V]`, built by iterating
    /// the native map and inserting each converted key→value. Unlike [`Shape::Dict`]
    /// (string-keyed only, returning a `BTreeMap<String, V>`), both key and value
    /// are arbitrary shapes and the result is a `Py<PyDict>` wrapped in a
    /// [`crate::ir_core::DictStub`] so the stub renders `dict[Kstub, Vstub]`.
    Map(Box<Shape>, Box<Shape>),
    /// A Rust tuple → a Python `tuple`, e.g. `(String, i128)`.
    Tuple(Vec<Shape>),
    /// A nested reflected struct/entity value → `Py<crate::sem::Wrapper>` via
    /// `Wrapper::build` (only valid for entities with no build-time extras).
    StructRef(Ident),
    Skip,
}

impl Shape {
    /// Whether emission touches the union builders (needs `py` + the model).
    fn needs_py(&self) -> bool {
        match self {
            Shape::UnionRef(_) | Shape::RewrapRef(..) | Shape::AstDef(_) | Shape::StructRef(_) => {
                true
            }
            Shape::Opt(s) | Shape::List(s) | Shape::Dict(s) => s.needs_py(),
            // Building the `PyDict` needs `py` regardless of key/value shapes.
            Shape::Map(_, _) => true,
            Shape::Tuple(v) => v.iter().any(Shape::needs_py),
            Shape::Span => true,
            _ => false,
        }
    }

    /// The Rust return type of a getter yielding this shape. `reg` resolves a
    /// `RewrapRef`'s concrete subclass (its static native type is that payload).
    fn ty(&self, reg: &UnionReg) -> TokenStream {
        match self {
            Shape::Bool => quote!(bool),
            Shape::I128 => quote!(i128),
            Shape::F64 => quote!(f64),
            Shape::Usize => quote!(i128),
            Shape::Str => quote!(String),
            Shape::Node => quote!(u32),
            Shape::Span => quote!(::pyo3::Py<crate::ir_core::Span>),
            Shape::UnionRef(name) => union_ref_ty(name),
            // A rewrapped bare payload is statically the variant's concrete type,
            // so it refines to `Py<Subclass>` rather than the broad union alias.
            Shape::RewrapRef(name, variant) => {
                let sub = rewrap_subclass(reg, name, variant);
                quote!(::pyo3::Py<crate::sem::#sub>)
            }
            Shape::Leaf(p) => quote!(#p),
            Shape::AstDef(id) => quote!(::pyo3::Py<crate::ast::#id>),
            Shape::Opt(s) => {
                let inner = s.ty(reg);
                quote!(Option<#inner>)
            }
            Shape::List(s) => {
                let inner = s.ty(reg);
                quote!(Vec<#inner>)
            }
            Shape::Dict(s) => {
                let inner = s.ty(reg);
                quote!(::std::collections::BTreeMap<String, #inner>)
            }
            Shape::Map(k, v) => {
                let kt = k.ty(reg);
                let vt = v.ty(reg);
                quote!(crate::ir_core::DictStub<#kt, #vt>)
            }
            Shape::Tuple(v) => {
                let tys = v.iter().map(|s| s.ty(reg));
                quote!((#(#tys),*))
            }
            Shape::StructRef(id) => quote!(::pyo3::Py<crate::sem::#id>),
            Shape::Skip => quote!(()),
        }
    }

    /// Emit an expression of type [`Shape::ty`] converting `vref`, a **already
    /// parenthesized** expression yielding a `&Native` reference to the
    /// field/return value (parenthesized so appended `.method()` binds to the
    /// whole reference, not its inner field access). `model`/`data` are
    /// place-expressions yielding the wrapper's `Py<Model>` handle and
    /// `Arc<ModelData>`. The expression may contain `?`, so the enclosing fn must
    /// return `PyResult`.
    fn expr(
        &self,
        vref: &TokenStream,
        model: &TokenStream,
        data: &TokenStream,
        reg: &UnionReg,
    ) -> TokenStream {
        match self {
            Shape::Bool | Shape::I128 | Shape::F64 => quote!(*#vref),
            Shape::Usize => quote!((*#vref as i128)),
            Shape::Str => quote!((#vref).to_string()),
            Shape::Node => quote!(#data.ids.get(#vref).copied().unwrap_or(0)),
            // `#vref` is a `&Span`; clone the backing model, deref the `Copy` span.
            Shape::Span => {
                quote!(::pyo3::Py::new(py, crate::ir_core::Span::new(#data.clone(), *#vref))?)
            }
            Shape::UnionRef(name) => {
                let f = union_ref_fn(name);
                let info = reg
                    .get(name)
                    .unwrap_or_else(|| panic!("unregistered union `{name}`"));
                // Arc-type / symbol handles take an owned clone; value / clone
                // handles take the native by reference.
                if info.handle.passes_owned() {
                    quote!(#f(#model, py, ::std::clone::Clone::clone(#vref))?)
                } else {
                    quote!(#f(#model, py, #vref)?)
                }
            }
            Shape::RewrapRef(name, variant) => {
                let build = union_build_fn(name);
                let sub = rewrap_subclass(reg, name, variant);
                let info = reg
                    .get(name)
                    .unwrap_or_else(|| panic!("unregistered union `{name}`"));
                let native = &info.native;
                let built = match info.handle {
                    Handle::ArcType => {
                        quote!(#build(#model, py, ::std::sync::Arc::new(#native::#variant(::std::clone::Clone::clone(#vref))))?)
                    }
                    Handle::Value | Handle::Clone => {
                        quote!(#build(#model, py, &#native::#variant(::std::clone::Clone::clone(#vref)))?)
                    }
                    Handle::Symbol => {
                        quote!(#build(#model, py, #native::#variant(::std::clone::Clone::clone(#vref)))?)
                    }
                };
                quote! {
                    #built
                        .into_bound(py)
                        .into_any()
                        .cast_into::<crate::sem::#sub>()?
                        .unbind()
                }
            }
            Shape::Leaf(p) => quote!(#p::from(#vref)),
            // `.node_id` is read as a bare field (not `fpp_ast::AstNode::id()`)
            // because this shape spans two kinds of native value unified only by
            // that field: a real `#[ast]` node (`node_id` is the reserved field the
            // macro injects, == `AstNode::id()`) and the `fpp_analysis`
            // `DefModuleStub` bridge (its own `node_id: Node`, which does *not* impl
            // `AstNode`). Field access also auto-derefs through an `Arc<DefX>`
            // payload, which a fully-qualified trait call would not.
            Shape::AstDef(id) => quote! {
                crate::model::Model::build(#model, py, #vref.node_id)?
                    .into_bound(py)
                    .into_any()
                    .cast_into::<crate::ast::#id>()?
                    .unbind()
            },
            Shape::Opt(s) => {
                let inner = s.expr(&quote!((v)), model, data, reg);
                quote!(match #vref.as_ref() { Some(v) => Some(#inner), None => None })
            }
            Shape::List(s) => {
                let inner = s.expr(&quote!((__e)), model, data, reg);
                quote!({
                    let mut __v = Vec::new();
                    for __e in #vref.iter() { __v.push(#inner); }
                    __v
                })
            }
            Shape::Dict(s) => {
                let inner = s.expr(&quote!((__e)), model, data, reg);
                quote!({
                    let mut __m = ::std::collections::BTreeMap::new();
                    for (__k, __e) in #vref.iter() { __m.insert(__k.clone(), #inner); }
                    __m
                })
            }
            Shape::Map(k, v) => {
                let kt = k.ty(reg);
                let vt = v.ty(reg);
                let kexpr = k.expr(&quote!((__k)), model, data, reg);
                let vexpr = v.expr(&quote!((__e)), model, data, reg);
                quote!({
                    let __d = ::pyo3::types::PyDict::new(py);
                    for (__k, __e) in #vref.iter() {
                        __d.set_item(#kexpr, #vexpr)?;
                    }
                    crate::ir_core::DictStub::<#kt, #vt>::new(__d.unbind())
                })
            }
            Shape::Tuple(v) => {
                let elems = v.iter().enumerate().map(|(i, s)| {
                    let idx = syn::Index::from(i);
                    s.expr(&quote!((&(#vref).#idx)), model, data, reg)
                });
                quote!((#(#elems),*))
            }
            Shape::StructRef(id) => {
                quote!(crate::sem::#id::build(#model, py, ::std::clone::Clone::clone(#vref))?)
            }
            Shape::Skip => quote!(()),
        }
    }
}

enum PayloadKind {
    Unit,
    /// Fields come from a matching `payload` decl (keyed by the subclass name).
    Struct,
    /// A single-value variant → one getter named `value` with this shape,
    /// projecting the bound payload directly (`x`).
    Value(Shape),
    /// A single-field tuple-struct payload → one getter named `value` with this
    /// shape, projecting the inner field (`x.0`), e.g. `IntegerValue(pub i128)`.
    Newtype(Shape),
    /// An inline struct variant (`Native::Variant { f1, f2, .. }`) → one getter
    /// per listed named field, matched by name. Used by the `clone`-handle
    /// `PortInstance` union whose variants are inline structs.
    StructVariant(Vec<FieldDecl>),
}

struct VariantDecl {
    native_variant: Ident,
    subclass: Ident,
    payload: PayloadKind,
}

/// A Python-marshallable scalar method argument.
#[derive(Clone, Copy)]
enum ScalarArg {
    I128,
    Bool,
    Usize,
    /// A borrowed `&str` native param, fed from an owned Python `String` via
    /// `#n.as_str()`.
    Str,
    /// An owned `String` native param, fed the Python `String` by value (`#n`).
    StringVal,
}

/// How a method parameter is supplied to the native call.
enum ArgKind {
    /// The injected `&Analysis` (context-dependent place-expr); NOT a Python
    /// parameter. Covers both the legacy bare `analysis` form and `<name>: analysis`.
    Analysis,
    /// A Python `Symbol` wrapper feeding a native `Symbol` param. `borrowed` picks
    /// the pass form: a borrowed `&Symbol` param takes `&#n.sym`; an owned `Symbol`
    /// param takes a clone (`#n.sym.clone()`) — the wrapper only lends its `sym`.
    Symbol { borrowed: bool },
    /// A Python scalar received directly.
    Scalar(ScalarArg),
}

/// A single method parameter.
struct MethodParam {
    name: Ident,
    kind: ArgKind,
}

struct MethodDecl {
    assoc: bool,
    name: Ident,
    params: Vec<MethodParam>,
    shape: Shape,
}

struct FieldDecl {
    name: Ident,
    shape: Shape,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Handle {
    ArcType,
    Value,
    Symbol,
    /// Store the union's native enum by value (matched via `&base.native`).
    Clone,
}

impl Handle {
    /// Whether the stored native is behind an `Arc` (matched via `.as_ref()`).
    fn is_arc(self) -> bool {
        matches!(self, Handle::ArcType)
    }
    /// Whether a `*_ref` builder for this handle takes an owned clone of the
    /// native value (Arc-type / symbol) rather than a shared reference
    /// (value / by-value clone).
    fn passes_owned(self) -> bool {
        matches!(self, Handle::ArcType | Handle::Symbol)
    }
    /// The base struct field type storing the native handle. `native` is the
    /// union's native enum path (from the DSL `native` clause); every handle
    /// stores that path — Arc-wrapped for `ArcType`, by value otherwise. No
    /// concrete `fpp_analysis` type name is baked in here.
    fn field_ty(self, native: &Path) -> TokenStream {
        match self {
            Handle::ArcType => quote!(::std::sync::Arc<#native>),
            Handle::Value | Handle::Symbol | Handle::Clone => quote!(#native),
        }
    }
}

/// A union's identity (`__eq__`/`__hash__`) directive. The method identifiers are
/// carried as DSL payloads (not hardcoded here) so a rename in `fpp_analysis`
/// regenerates a still-correct call site.
enum Identity {
    /// No generated identity — default PyO3 object identity.
    None,
    /// Native `==` + hash by node id (`accessor.node()` via `SymbolInterface`).
    Node,
    /// Value equality + hash by node id via the carried `(eq_assoc_fn,
    /// node_id_method)` idents: `Native::<eq>(&a, &b)` for `__eq__` and
    /// `accessor.<node_id>()` for `__hash__` (the arc-shared `Type` quirk).
    Identical(Ident, Ident),
}

/// A union's `__repr__` directive. All forms render `<Alias …>`; the `…` is the
/// native variant discriminant name, optionally followed by a quoted qualified /
/// unqualified name resolved via the carried method identifier.
enum Repr {
    /// No generated repr — hand-written elsewhere.
    None,
    /// `<Alias Variant>`.
    Variant,
    /// `<Alias Variant 'qualified'>` — the `Ident` names the `Analysis` method
    /// called as `self.data.analysis.<method>(&accessor)`.
    VariantQualified(Ident),
    /// `<Alias Variant 'unqualified'>` — the `Ident` names the accessor method
    /// called as `accessor.<method>()`.
    VariantUnqualified(Ident),
}

struct UnionDecl {
    py: Ident,
    native: Path,
    handle: Handle,
    alias: LitStr,
    accessor: Ident,
    include_base: bool,
    /// When set, `build_*` is hand-written (a per-hierarchy quirk, e.g. the
    /// unknown `Type`); otherwise the macro generates a default dispatch build.
    custom_build: bool,
    /// When set, emit a base `loc` getter resolving the source location from the
    /// stored native's node id (`accessor.node()` via `SymbolInterface`).
    loc_from_node: bool,
    /// Identity directive (`__eq__`/`__hash__`).
    identity: Identity,
    /// `__repr__` directive.
    repr: Repr,
    variants: Vec<VariantDecl>,
    methods: Vec<MethodDecl>,
}

struct PayloadDecl {
    name: Ident,
    #[allow(dead_code)]
    native: Path,
    fields: Vec<FieldDecl>,
    methods: Vec<MethodDecl>,
}

/// How a standalone `entity` item stores + reaches its native value: a
/// `#[pyclass(frozen)]` holding a native `Clone` (+ any build-time extra scalars
/// its parent supplies), *not* a union subclass. Field/method getters project the
/// stored `field`; `extras` are plain stored scalars set by the `build`
/// constructor.
enum EntityHandle {
    Clone { field: Ident },
}

/// A clone-`entity`'s `__eq__`/`__hash__` directive.
enum EntityIdentity {
    /// No generated identity — default PyO3 object identity.
    None,
    /// Native `==` + hash by `data.ids[native.node()]` (native: `SymbolInterface`).
    Node,
    /// `__eq__`/`__hash__` from `self.<field>.<method>()` (a `String`); the carried
    /// `Ident` names that method (e.g. `qualified_name`).
    QualifiedName(Ident),
    /// Delegate to the native's `PartialEq`/`Hash` (native: `Copy + Hash + Eq`).
    RawHandle,
}

/// A standalone `entity` item (see [`EntityHandle`] for the two storage shapes).
struct EntityDecl {
    py: Ident,
    native: Path,
    handle: EntityHandle,
    /// `__eq__`/`__hash__` directive (clone-handle entities only).
    identity: EntityIdentity,
    extras: Vec<FieldDecl>,
    fields: Vec<FieldDecl>,
    methods: Vec<MethodDecl>,
}

/// The binding pattern of a leaf-enum native variant, controlling the `From`
/// match arm (a fieldless mirror discards any payload).
#[derive(Clone, Copy)]
enum LeafPattern {
    /// `Native::V` (no fields).
    Unit,
    /// `Native::V(..)` (tuple/newtype fields).
    Tuple,
    /// `Native::V { .. }` (named fields).
    Struct,
}

/// A leaf-enum mirror: a fieldless `#[pyclass(eq, eq_int)]` Python enum plus a
/// `From<&native>` mapping each native variant onto it (the discriminant only;
/// any payload is exposed by dedicated getters elsewhere).
struct LeafEnumDecl {
    py: Ident,
    native: Path,
    /// `(variant ident, its native binding pattern)`.
    variants: Vec<(Ident, LeafPattern)>,
}

/// The root `analysis native <path> { fields{} methods{} }` section, declaring
/// the `Analysis` wrapper over `data.analysis`.
struct AnalysisDecl {
    native: Path,
    fields: Vec<FieldDecl>,
    methods: Vec<MethodDecl>,
}

struct Dsl {
    unions: Vec<UnionDecl>,
    payloads: Vec<PayloadDecl>,
    entities: Vec<EntityDecl>,
    leaf_enums: Vec<LeafEnumDecl>,
    analysis: Vec<AnalysisDecl>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

impl Parse for Shape {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let kw: Ident = input.parse()?;
        let s = kw.to_string();
        Ok(match s.as_str() {
            "bool" => Shape::Bool,
            "i128" => Shape::I128,
            "f64" => Shape::F64,
            // Any narrower integer is widened to a Python int via `as i128`.
            "usize" | "isize" | "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" => {
                Shape::Usize
            }
            "str" => Shape::Str,
            "node" => Shape::Node,
            "span" => Shape::Span,
            "skip" => Shape::Skip,
            "opt" | "list" | "dict" => {
                let content;
                parenthesized!(content in input);
                let inner: Shape = content.parse()?;
                match s.as_str() {
                    "opt" => Shape::Opt(Box::new(inner)),
                    "list" => Shape::List(Box::new(inner)),
                    _ => Shape::Dict(Box::new(inner)),
                }
            }
            // `map(<key_shape>, <value_shape>)` → a real Python `dict[K, V]`.
            "map" => {
                let content;
                parenthesized!(content in input);
                let key: Shape = content.parse()?;
                content.parse::<Token![,]>()?;
                let val: Shape = content.parse()?;
                Shape::Map(Box::new(key), Box::new(val))
            }
            "tuple" => {
                let content;
                parenthesized!(content in input);
                let elems = content.parse_terminated(Shape::parse, Token![,])?;
                Shape::Tuple(elems.into_iter().collect())
            }
            "leaf" => {
                let content;
                parenthesized!(content in input);
                Shape::Leaf(content.parse()?)
            }
            "astdef" => {
                let content;
                parenthesized!(content in input);
                Shape::AstDef(content.parse()?)
            }
            "entity" => {
                let content;
                parenthesized!(content in input);
                Shape::StructRef(content.parse()?)
            }
            // A closed union referenced by its Python name, e.g. `union(InterfaceInstance)`.
            "union" => {
                let content;
                parenthesized!(content in input);
                let name: Ident = content.parse()?;
                Shape::UnionRef(name.to_string())
            }
            "rewrap" => {
                let content;
                parenthesized!(content in input);
                let path: Path = content.parse()?;
                // `<UnionPyName>::<Variant>` — the union is any declared union.
                let name = path.segments[0].ident.to_string();
                let variant = path.segments[1].ident.clone();
                Shape::RewrapRef(name, variant)
            }
            other => {
                return Err(syn::Error::new(
                    kw.span(),
                    format!("unknown shape `{other}`"),
                ));
            }
        })
    }
}

impl Parse for FieldDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let shape: Shape = input.parse()?;
        Ok(FieldDecl { name, shape })
    }
}

impl Parse for MethodParam {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;
            let kind_id: Ident = input.parse()?;
            let kind = match kind_id.to_string().as_str() {
                "analysis" => ArgKind::Analysis,
                "symbol" => ArgKind::Symbol { borrowed: true },
                "symbol_owned" => ArgKind::Symbol { borrowed: false },
                "i128" => ArgKind::Scalar(ScalarArg::I128),
                "bool" => ArgKind::Scalar(ScalarArg::Bool),
                "str" => ArgKind::Scalar(ScalarArg::Str),
                "string" => ArgKind::Scalar(ScalarArg::StringVal),
                "usize" => ArgKind::Scalar(ScalarArg::Usize),
                other => {
                    return Err(syn::Error::new(
                        kind_id.span(),
                        format!(
                            "unknown arg kind `{other}` (expected \
                             analysis/symbol/symbol_owned/i128/bool/str/string/usize)"
                        ),
                    ));
                }
            };
            Ok(MethodParam { name, kind })
        } else {
            // Legacy bare `analysis`: the injected `&Analysis` with no `: kind`.
            if name != "analysis" {
                return Err(syn::Error::new(
                    name.span(),
                    "a bare method arg must be `analysis` (the injected &Analysis); \
                     other args use `<name>: <kind>`",
                ));
            }
            Ok(MethodParam {
                name,
                kind: ArgKind::Analysis,
            })
        }
    }
}

impl Parse for MethodDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let assoc = if input.peek(Ident) && input.fork().parse::<Ident>()? == "assoc" {
            input.parse::<Ident>()?;
            true
        } else {
            false
        };
        let name: Ident = input.parse()?;
        let params = if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            content
                .parse_terminated(MethodParam::parse, Token![,])?
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };
        input.parse::<Token![->]>()?;
        let shape: Shape = input.parse()?;
        Ok(MethodDecl {
            assoc,
            name,
            params,
            shape,
        })
    }
}

impl Parse for VariantDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let native_variant: Ident = input.parse()?;
        input.parse::<Token![=>]>()?;
        let subclass: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        // `struct { … }` | `unit` | `payload` | `newtype(<shape>)` | <shape>
        // (`struct` is a reserved keyword, so it is peeked separately.)
        let payload = if input.peek(Token![struct]) {
            input.parse::<Token![struct]>()?;
            let content;
            braced!(content in input);
            let fields = content.parse_terminated(FieldDecl::parse, Token![,])?;
            PayloadKind::StructVariant(fields.into_iter().collect())
        } else if input.peek(Ident) {
            let fork = input.fork();
            let kw: Ident = fork.parse()?;
            match kw.to_string().as_str() {
                "unit" => {
                    input.parse::<Ident>()?;
                    PayloadKind::Unit
                }
                "payload" => {
                    input.parse::<Ident>()?;
                    PayloadKind::Struct
                }
                "newtype" => {
                    input.parse::<Ident>()?;
                    let content;
                    parenthesized!(content in input);
                    PayloadKind::Newtype(content.parse()?)
                }
                _ => PayloadKind::Value(input.parse()?),
            }
        } else {
            PayloadKind::Value(input.parse()?)
        };
        Ok(VariantDecl {
            native_variant,
            subclass,
            payload,
        })
    }
}

/// Parse a `<section> { <T>,* }` braced, comma-terminated list.
fn parse_section<T: Parse>(input: ParseStream, name: &str) -> syn::Result<Vec<T>> {
    let kw: Ident = input.parse()?;
    if kw != name {
        return Err(syn::Error::new(kw.span(), format!("expected `{name}`")));
    }
    let content;
    braced!(content in input);
    let items = content.parse_terminated(T::parse, Token![,])?;
    Ok(items.into_iter().collect())
}

/// Peek whether the next section keyword matches `name` (for optional sections).
fn peek_section(input: ParseStream, name: &str) -> bool {
    input.peek(Ident)
        && input
            .fork()
            .parse::<Ident>()
            .map(|i| i == name)
            .unwrap_or(false)
}

impl Parse for UnionDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let py: Ident = input.parse()?;
        expect_kw(input, "native")?;
        let native: Path = input.parse()?;
        expect_kw(input, "handle")?;
        let handle_id: Ident = input.parse()?;
        let handle = match handle_id.to_string().as_str() {
            "arc_type" => Handle::ArcType,
            "value" => Handle::Value,
            "symbol" => Handle::Symbol,
            "clone" => Handle::Clone,
            other => {
                return Err(syn::Error::new(
                    handle_id.span(),
                    format!("unknown handle `{other}`"),
                ));
            }
        };
        expect_kw(input, "alias")?;
        let alias: LitStr = input.parse()?;
        expect_kw(input, "accessor")?;
        let accessor: Ident = input.parse()?;
        let include_base = if peek_kw(input, "include_base") {
            input.parse::<Ident>()?;
            true
        } else {
            false
        };
        let custom_build = if peek_kw(input, "custom_build") {
            input.parse::<Ident>()?;
            true
        } else {
            false
        };
        let loc_from_node = if peek_kw(input, "loc_from_node") {
            input.parse::<Ident>()?;
            true
        } else {
            false
        };
        let identity = if peek_kw(input, "identity") {
            input.parse::<Ident>()?; // consume `identity`
            let mode: Ident = input.parse()?;
            match mode.to_string().as_str() {
                "node" => Identity::Node,
                // `identical(<eq_fn>, <node_id_method>)` — payload optional, with
                // the historical `(identical, def_node_id)` default so an older
                // (payload-free) `defs.rs` still parses.
                "identical" => {
                    let (eq_fn, node_id) = parse_ident_pair_payload(
                        input,
                        || format_ident!("identical"),
                        || format_ident!("def_node_id"),
                    )?;
                    Identity::Identical(eq_fn, node_id)
                }
                other => {
                    return Err(syn::Error::new(
                        mode.span(),
                        format!("unknown identity mode `{other}` (expected node/identical)"),
                    ));
                }
            }
        } else {
            Identity::None
        };
        let repr = if peek_kw(input, "repr") {
            input.parse::<Ident>()?; // consume `repr`
            let mode: Ident = input.parse()?;
            match mode.to_string().as_str() {
                "variant" => Repr::Variant,
                // `variant_qualified(<method>)` / `variant_unqualified(<method>)` —
                // payload optional, defaulting to the historical method names so an
                // older (payload-free) `defs.rs` still parses.
                "variant_qualified" => Repr::VariantQualified(parse_ident_payload(input, || {
                    format_ident!("get_qualified_name")
                })?),
                "variant_unqualified" => {
                    Repr::VariantUnqualified(parse_ident_payload(input, || {
                        format_ident!("get_unqualified_name")
                    })?)
                }
                other => {
                    return Err(syn::Error::new(
                        mode.span(),
                        format!(
                            "unknown repr mode `{other}` \
                             (expected variant/variant_qualified/variant_unqualified)"
                        ),
                    ));
                }
            }
        } else {
            Repr::None
        };
        let body;
        braced!(body in input);
        let variants = parse_section::<VariantDecl>(&body, "variants")?;
        let methods = if peek_section(&body, "methods") {
            parse_section::<MethodDecl>(&body, "methods")?
        } else {
            Vec::new()
        };
        Ok(UnionDecl {
            py,
            native,
            handle,
            alias,
            accessor,
            include_base,
            custom_build,
            loc_from_node,
            identity,
            repr,
            variants,
            methods,
        })
    }
}

impl Parse for PayloadDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        expect_kw(input, "native")?;
        let native: Path = input.parse()?;
        let body;
        braced!(body in input);
        let fields = if peek_section(&body, "fields") {
            parse_section::<FieldDecl>(&body, "fields")?
        } else {
            Vec::new()
        };
        let methods = if peek_section(&body, "methods") {
            parse_section::<MethodDecl>(&body, "methods")?
        } else {
            Vec::new()
        };
        Ok(PayloadDecl {
            name,
            native,
            fields,
            methods,
        })
    }
}

impl Parse for EntityDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let py: Ident = input.parse()?;
        expect_kw(input, "native")?;
        let native: Path = input.parse()?;
        // An optional `field <ident>` names the clone handle's stored field (else
        // the default `native`).
        let handle = if peek_kw(input, "field") {
            input.parse::<Ident>()?; // consume `field`
            EntityHandle::Clone {
                field: input.parse()?,
            }
        } else {
            EntityHandle::Clone {
                field: format_ident!("native"),
            }
        };
        // Optional `identity <mode>` directive (clone-handle entities only).
        let identity = if peek_kw(input, "identity") {
            input.parse::<Ident>()?; // consume `identity`
            let mode: Ident = input.parse()?;
            match mode.to_string().as_str() {
                "node" => EntityIdentity::Node,
                // `qualified_name(<method>)` — payload optional, defaulting to the
                // historical `qualified_name` so an older `defs.rs` still parses.
                "qualified_name" => {
                    EntityIdentity::QualifiedName(parse_ident_payload(input, || {
                        format_ident!("qualified_name")
                    })?)
                }
                "raw_handle" => EntityIdentity::RawHandle,
                other => {
                    return Err(syn::Error::new(
                        mode.span(),
                        format!(
                            "unknown identity mode `{other}` \
                             (expected node/qualified_name/raw_handle)"
                        ),
                    ));
                }
            }
        } else {
            EntityIdentity::None
        };
        let body;
        braced!(body in input);
        let extras = if peek_section(&body, "extras") {
            parse_section::<FieldDecl>(&body, "extras")?
        } else {
            Vec::new()
        };
        let fields = if peek_section(&body, "fields") {
            parse_section::<FieldDecl>(&body, "fields")?
        } else {
            Vec::new()
        };
        let methods = if peek_section(&body, "methods") {
            parse_section::<MethodDecl>(&body, "methods")?
        } else {
            Vec::new()
        };
        Ok(EntityDecl {
            py,
            native,
            handle,
            identity,
            extras,
            fields,
            methods,
        })
    }
}

impl Parse for AnalysisDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        expect_kw(input, "native")?;
        let native: Path = input.parse()?;
        let body;
        braced!(body in input);
        let fields = if peek_section(&body, "fields") {
            parse_section::<FieldDecl>(&body, "fields")?
        } else {
            Vec::new()
        };
        let methods = if peek_section(&body, "methods") {
            parse_section::<MethodDecl>(&body, "methods")?
        } else {
            Vec::new()
        };
        Ok(AnalysisDecl {
            native,
            fields,
            methods,
        })
    }
}

impl Parse for LeafEnumDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let py: Ident = input.parse()?;
        expect_kw(input, "native")?;
        let native: Path = input.parse()?;
        let body;
        braced!(body in input);
        // `<Variant>: <unit|tuple|struct>,` entries.
        let entries = body.parse_terminated(
            |s: ParseStream| -> syn::Result<(Ident, LeafPattern)> {
                let variant: Ident = s.parse()?;
                s.parse::<Token![:]>()?;
                // `struct` is a reserved keyword, so it is peeked separately.
                let pat = if s.peek(Token![struct]) {
                    s.parse::<Token![struct]>()?;
                    LeafPattern::Struct
                } else {
                    let kind: Ident = s.parse()?;
                    match kind.to_string().as_str() {
                        "unit" => LeafPattern::Unit,
                        "tuple" => LeafPattern::Tuple,
                        other => {
                            return Err(syn::Error::new(
                                kind.span(),
                                format!("unknown leaf-enum pattern `{other}`"),
                            ));
                        }
                    }
                };
                Ok((variant, pat))
            },
            Token![,],
        )?;
        Ok(LeafEnumDecl {
            py,
            native,
            variants: entries.into_iter().collect(),
        })
    }
}

fn expect_kw(input: ParseStream, name: &str) -> syn::Result<()> {
    let kw: Ident = input.parse()?;
    if kw != name {
        return Err(syn::Error::new(kw.span(), format!("expected `{name}`")));
    }
    Ok(())
}

fn peek_kw(input: ParseStream, name: &str) -> bool {
    input.peek(Ident)
        && input
            .fork()
            .parse::<Ident>()
            .map(|i| i == name)
            .unwrap_or(false)
}

/// Parse an optional `(<ident>)` directive payload. Absent → `default()`. Lets a
/// directive carry the `fpp_analysis` method name it calls, while keeping the
/// payload optional so a payload-free `defs.rs` still parses (back-compat).
fn parse_ident_payload(input: ParseStream, default: impl FnOnce() -> Ident) -> syn::Result<Ident> {
    if input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in input);
        content.parse()
    } else {
        Ok(default())
    }
}

/// Parse an optional `(<ident>, <ident>)` directive payload. Absent → the two
/// defaults. Same back-compat rationale as [`parse_ident_payload`].
fn parse_ident_pair_payload(
    input: ParseStream,
    default_a: impl FnOnce() -> Ident,
    default_b: impl FnOnce() -> Ident,
) -> syn::Result<(Ident, Ident)> {
    if input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in input);
        let a: Ident = content.parse()?;
        content.parse::<Token![,]>()?;
        let b: Ident = content.parse()?;
        Ok((a, b))
    } else {
        Ok((default_a(), default_b()))
    }
}

impl Parse for Dsl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut unions = Vec::new();
        let mut payloads = Vec::new();
        let mut entities = Vec::new();
        let mut leaf_enums = Vec::new();
        let mut analysis = Vec::new();
        while !input.is_empty() {
            let kw: Ident = input.parse()?;
            match kw.to_string().as_str() {
                "union" => unions.push(input.parse()?),
                "payload" => payloads.push(input.parse()?),
                "entity" => entities.push(input.parse()?),
                "leaf_enum" => leaf_enums.push(input.parse()?),
                "analysis" => analysis.push(input.parse()?),
                other => {
                    return Err(syn::Error::new(
                        kw.span(),
                        format!(
                            "unknown section `{other}` \
                             (expected union/payload/entity/leaf_enum/analysis)"
                        ),
                    ));
                }
            }
        }
        Ok(Dsl {
            unions,
            payloads,
            entities,
            leaf_enums,
            analysis,
        })
    }
}

// ---------------------------------------------------------------------------
// Emit
// ---------------------------------------------------------------------------

fn snake(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Emit a getter (or, when it has real Python parameters, a callable method) over
/// a base-handle union member. `body` reads `base`/`self`; `via_super` selects
/// `PyRef` vs `&self` receiver. `extra_params` are the extra Python-facing fn
/// parameters (leading `,` included) — when non-empty the item is a callable
/// method (`is_method`), not a `#[getter]` property.
struct Getter {
    name: Ident,
    shape_ty: TokenStream,
    needs_py: bool,
    body: TokenStream,
    via_super: bool,
    extra_params: TokenStream,
    is_method: bool,
}

impl Getter {
    /// A plain field getter (no extra params, always a `#[getter]`).
    fn field(name: Ident, shape_ty: TokenStream, needs_py: bool, body: TokenStream) -> Self {
        Getter {
            name,
            shape_ty,
            needs_py,
            body,
            via_super: false,
            extra_params: quote!(),
            is_method: false,
        }
    }
}

/// Whether `s` is a Python (hard) keyword — one that cannot appear as a bare
/// `def <name>` in a `.pyi` stub, nor be read as a plain attribute. Soft keywords
/// (`match`/`case`/`type`) are omitted: they parse fine as identifiers.
fn is_py_keyword(s: &str) -> bool {
    matches!(
        s,
        "False"
            | "None"
            | "True"
            | "and"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "break"
            | "class"
            | "continue"
            | "def"
            | "del"
            | "elif"
            | "else"
            | "except"
            | "finally"
            | "for"
            | "from"
            | "global"
            | "if"
            | "import"
            | "in"
            | "is"
            | "lambda"
            | "nonlocal"
            | "not"
            | "or"
            | "pass"
            | "raise"
            | "return"
            | "try"
            | "while"
            | "with"
            | "yield"
    )
}

/// The Python-facing name for a getter/method: a native field/method whose name
/// collides with a Python keyword (e.g. `Connection::from`) is exposed with a
/// trailing underscore (`from_`), matching PEP 8. Only the emitted fn name is
/// rewritten — getter bodies read the native value through the caller-built
/// access expression, so the underlying field/method reference is unaffected.
fn py_getter_ident(name: &Ident) -> Ident {
    if is_py_keyword(&name.to_string()) {
        format_ident!("{}_", name)
    } else {
        name.clone()
    }
}

fn getter_tokens(g: &Getter) -> TokenStream {
    let name = py_getter_ident(&g.name);
    let ty = &g.shape_ty;
    let body = &g.body;
    let extra = &g.extra_params;
    let py_param = if g.needs_py {
        quote!(, py: ::pyo3::Python<'_>)
    } else {
        quote!()
    };
    let attr = if g.is_method {
        quote!()
    } else {
        quote!(#[getter])
    };
    if g.via_super {
        quote! {
            #attr
            fn #name(self_: ::pyo3::PyRef<'_, Self> #py_param #extra) -> ::pyo3::PyResult<#ty> {
                let base = self_.as_super();
                #body
            }
        }
    } else {
        quote! {
            #attr
            fn #name(&self #py_param #extra) -> ::pyo3::PyResult<#ty> {
                #body
            }
        }
    }
}

/// Build the Python-facing extra signature params (leading `,` included) and the
/// native call-arg list for a method's parameters. `analysis_access` is the
/// place-expr for the injected `&Analysis` in this context (e.g.
/// `&self.data.analysis`). The `bool` is whether any real (non-`analysis`) Python
/// param is present — i.e. the item must be a callable method, not a `#[getter]`.
fn method_arg_parts(
    params: &[MethodParam],
    analysis_access: &TokenStream,
) -> (TokenStream, Vec<TokenStream>, bool) {
    let mut sig: Vec<TokenStream> = Vec::new();
    let mut call: Vec<TokenStream> = Vec::new();
    let mut has_py_param = false;
    for p in params {
        match &p.kind {
            ArgKind::Analysis => call.push(analysis_access.clone()),
            ArgKind::Symbol { borrowed } => {
                has_py_param = true;
                let n = &p.name;
                sig.push(quote!(#n: ::pyo3::PyRef<'_, crate::sem::Symbol>));
                if *borrowed {
                    call.push(quote!(&#n.sym));
                } else {
                    call.push(quote!(::std::clone::Clone::clone(&#n.sym)));
                }
            }
            ArgKind::Scalar(s) => {
                has_py_param = true;
                let n = &p.name;
                let (ty, pass) = match s {
                    ScalarArg::I128 => (quote!(i128), quote!(#n)),
                    ScalarArg::Bool => (quote!(bool), quote!(#n)),
                    ScalarArg::Usize => (quote!(usize), quote!(#n)),
                    ScalarArg::Str => (quote!(::std::string::String), quote!(#n.as_str())),
                    ScalarArg::StringVal => (quote!(::std::string::String), quote!(#n)),
                };
                sig.push(quote!(#n: #ty));
                call.push(pass);
            }
        }
    }
    let sig_ts = if sig.is_empty() {
        quote!()
    } else {
        quote!(#(, #sig)*)
    };
    (sig_ts, call, has_py_param)
}

/// Emit the base method getters (shared by every subclass via inheritance).
fn emit_union_methods(u: &UnionDecl, reg: &UnionReg) -> Vec<TokenStream> {
    let accessor = &u.accessor;
    let native = &u.native;
    let model = quote!(&self.model);
    let data = quote!(self.data);
    let mut out: Vec<TokenStream> = Vec::new();
    // A `loc_from_node` union resolves its source location from the stored
    // native's node id (the native carries a node, not a span). `ModelData::loc`
    // takes the node's span inside its own `run_ref` scope.
    if u.loc_from_node {
        out.push(quote! {
            /// The source location of this element's definition.
            #[getter]
            fn loc(&self) -> ::std::option::Option<crate::ir_core::Loc> {
                self.data.loc(self.#accessor.node())
            }
        });
    }
    let analysis_access = quote!(&self.data.analysis);
    out.extend(u.methods.iter().map(|m| {
        let mname = &m.name;
        let (extra, call_args, is_method) = method_arg_parts(&m.params, &analysis_access);
        let call = if m.assoc {
            // Associated fn over `&Arc<Self>` / `&Self`, e.g. underlying_type.
            quote!(#native::#mname(&self.#accessor #(, #call_args)*))
        } else {
            // `&self` method (the Arc/enum derefs to the receiver).
            quote!(self.#accessor.#mname(#(#call_args),*))
        };
        let vref = quote!((&__r));
        let expr = m.shape.expr(&vref, &model, &data, reg);
        let ty = m.shape.ty(reg);
        // The native call may read the retained compiler context (annotations,
        // `Node::span`), so it runs under `run_ref`. This enters the context only
        // for the duration of the call/conversion; the conversion itself never
        // re-enters `run_ref` (spans resolve lazily), so nesting cannot occur.
        let body = quote! {
            ::fpp_core::run_ref(&#data.ctx, || {
                let __r = #call;
                Ok(#expr)
            })
        };
        getter_tokens(&Getter {
            name: mname.clone(),
            shape_ty: ty,
            needs_py: m.shape.needs_py(),
            body,
            via_super: false,
            extra_params: extra,
            is_method,
        })
    }));
    out
}

/// Emit a subclass's field getters (projected through the base handle).
fn emit_subclass_getters(
    u: &UnionDecl,
    v: &VariantDecl,
    payload: Option<&PayloadDecl>,
    reg: &UnionReg,
) -> Vec<TokenStream> {
    let accessor = &u.accessor;
    let native = &u.native;
    let variant = &v.native_variant;
    let model = quote!(&base.model);
    let data = quote!(base.data);
    // Scrutinee: `&Native` from the base handle.
    let scrut = if u.handle.is_arc() {
        quote!(base.#accessor.as_ref())
    } else {
        quote!(&base.#accessor)
    };

    let emit_one = |gname: &Ident, shape: &Shape, field_access: &TokenStream| -> TokenStream {
        let expr = shape.expr(field_access, &model, &data, reg);
        let ty = shape.ty(reg);
        let body = quote! {
            match #scrut {
                #native::#variant(x) => Ok(#expr),
                _ => unreachable!(),
            }
        };
        getter_tokens(&Getter {
            name: gname.clone(),
            shape_ty: ty,
            needs_py: shape.needs_py(),
            body,
            via_super: true,
            extra_params: quote!(),
            is_method: false,
        })
    };

    match &v.payload {
        PayloadKind::Unit => Vec::new(),
        PayloadKind::Value(shape) => {
            // A single-value variant: one getter, payload bound as `x`. An
            // `astdef` payload (a `Symbol` variant's `Arc<DefX>`) is named
            // `definition` and typed to the concrete AST wrapper; otherwise `value`.
            let gname = if matches!(shape, Shape::AstDef(_)) {
                format_ident!("definition")
            } else {
                format_ident!("value")
            };
            vec![emit_one(&gname, shape, &quote!((x)))]
        }
        PayloadKind::Newtype(shape) => {
            // A single-field tuple-struct payload: one getter `value` over `x.0`.
            let gname = format_ident!("value");
            vec![emit_one(&gname, shape, &quote!((&x.0)))]
        }
        PayloadKind::StructVariant(fields) => {
            // Inline struct variant: bind each listed field by name, matching
            // `&Native::Variant { field, .. }` (not the tuple `Variant(x)` form).
            let mut out = Vec::new();
            for f in fields {
                if matches!(f.shape, Shape::Skip) {
                    continue;
                }
                let fname = &f.name;
                let expr = f.shape.expr(&quote!((#fname)), &model, &data, reg);
                let ty = f.shape.ty(reg);
                let body = quote! {
                    match #scrut {
                        #native::#variant { #fname, .. } => Ok(#expr),
                        _ => unreachable!(),
                    }
                };
                out.push(getter_tokens(&Getter {
                    name: fname.clone(),
                    shape_ty: ty,
                    needs_py: f.shape.needs_py(),
                    body,
                    via_super: true,
                    extra_params: quote!(),
                    is_method: false,
                }));
            }
            out
        }
        PayloadKind::Struct => {
            let p = payload.expect("payload decl present for a `payload` variant");
            let mut out = Vec::new();
            for f in &p.fields {
                if matches!(f.shape, Shape::Skip) {
                    continue;
                }
                let access = {
                    let fname = &f.name;
                    quote!((&x.#fname))
                };
                out.push(emit_one(&f.name, &f.shape, &access));
            }
            // Payload struct methods (e.g. a `&self` accessor) project the payload.
            let analysis_access = quote!(&base.data.analysis);
            for m in &p.methods {
                let mname = &m.name;
                let (extra, call_args, is_method) = method_arg_parts(&m.params, &analysis_access);
                let call = if m.assoc {
                    let pnative = &p.native;
                    quote!(#pnative::#mname(x #(, #call_args)*))
                } else {
                    quote!(x.#mname(#(#call_args),*))
                };
                let vref = quote!((&__r));
                let expr = m.shape.expr(&vref, &model, &data, reg);
                let ty = m.shape.ty(reg);
                // The native call may read the retained compiler context, so it
                // runs under `run_ref` (see `emit_union_methods`).
                let body = quote! {
                    match #scrut {
                        #native::#variant(x) => ::fpp_core::run_ref(&#data.ctx, || {
                            let __r = #call;
                            Ok(#expr)
                        }),
                        _ => unreachable!(),
                    }
                };
                out.push(getter_tokens(&Getter {
                    name: mname.clone(),
                    shape_ty: ty,
                    needs_py: m.shape.needs_py(),
                    body,
                    via_super: true,
                    extra_params: extra,
                    is_method,
                }));
            }
            out
        }
    }
}

/// The base scrutinee (`&Native`) matching the stored handle (for the `__repr__`
/// kind-name match).
fn union_base_scrut(u: &UnionDecl) -> TokenStream {
    let accessor = &u.accessor;
    if u.handle.is_arc() {
        quote!(self.#accessor.as_ref())
    } else {
        quote!(&self.#accessor)
    }
}

/// The `<Native>::<Variant><pattern>` match pattern for a variant (payload
/// discarded — only the discriminant is named).
fn variant_match_pattern(native: &Path, v: &VariantDecl) -> TokenStream {
    let variant = &v.native_variant;
    match &v.payload {
        PayloadKind::Unit => quote!(#native::#variant),
        PayloadKind::StructVariant(_) => quote!(#native::#variant { .. }),
        _ => quote!(#native::#variant(..)),
    }
}

/// Emit the private `sem_repr_kind` helper (the native variant discriminant name),
/// used only by a generated `__repr__`. Empty when no repr is generated.
fn emit_union_kind_helper(u: &UnionDecl) -> TokenStream {
    if matches!(u.repr, Repr::None) {
        return quote!();
    }
    let base = &u.py;
    let native = &u.native;
    let scrut = union_base_scrut(u);
    let arms = u.variants.iter().map(|v| {
        let pat = variant_match_pattern(native, v);
        let name = v.native_variant.to_string();
        quote!(#pat => #name)
    });
    quote! {
        impl #base {
            /// The native variant discriminant name (for `__repr__`).
            fn sem_repr_kind(&self) -> &'static str {
                match #scrut {
                    #(#arms),*
                }
            }
        }
    }
}

/// Emit the generated `__repr__` method (empty when hand-written).
fn emit_union_repr(u: &UnionDecl) -> TokenStream {
    let accessor = &u.accessor;
    let alias = u.alias.value();
    match &u.repr {
        Repr::None => quote!(),
        Repr::Variant => {
            let fmt = format!("<{alias} {{}}>");
            quote! {
                fn __repr__(&self) -> ::std::string::String {
                    format!(#fmt, self.sem_repr_kind())
                }
            }
        }
        Repr::VariantQualified(method) => {
            let fmt = format!("<{alias} {{}} '{{}}'>");
            quote! {
                fn __repr__(&self) -> ::std::string::String {
                    format!(
                        #fmt,
                        self.sem_repr_kind(),
                        self.data.analysis.#method(&self.#accessor)
                    )
                }
            }
        }
        Repr::VariantUnqualified(method) => {
            let fmt = format!("<{alias} {{}} '{{}}'>");
            quote! {
                fn __repr__(&self) -> ::std::string::String {
                    format!(#fmt, self.sem_repr_kind(), self.#accessor.#method())
                }
            }
        }
    }
}

/// Emit the generated `__eq__`/`__hash__` methods (empty when default identity).
fn emit_union_identity(u: &UnionDecl) -> TokenStream {
    let base = &u.py;
    let accessor = &u.accessor;
    let native = &u.native;
    match &u.identity {
        Identity::None => quote!(),
        Identity::Node => quote! {
            fn __eq__(&self, other: &::pyo3::Bound<'_, ::pyo3::PyAny>) -> bool {
                match other.cast::<#base>() {
                    ::std::result::Result::Ok(o) => self.#accessor == o.borrow().#accessor,
                    ::std::result::Result::Err(_) => false,
                }
            }
            fn __hash__(&self) -> u64 {
                self.data.ids.get(&self.#accessor.node()).copied().unwrap_or(0) as u64
            }
        },
        Identity::Identical(eq_fn, node_id) => quote! {
            fn __eq__(&self, other: &::pyo3::Bound<'_, ::pyo3::PyAny>) -> bool {
                match other.cast::<#base>() {
                    ::std::result::Result::Ok(o) => {
                        #native::#eq_fn(&self.#accessor, &o.borrow().#accessor)
                    }
                    ::std::result::Result::Err(_) => false,
                }
            }
            fn __hash__(&self) -> u64 {
                match self.#accessor.#node_id() {
                    ::std::option::Option::Some(n) => {
                        self.data.ids.get(&n).copied().unwrap_or(0) as u64
                    }
                    ::std::option::Option::None => 0,
                }
            }
        },
    }
}

fn emit_union(
    u: &UnionDecl,
    payloads: &[PayloadDecl],
    reg: &UnionReg,
) -> (
    TokenStream,
    Vec<TokenStream>,
    Vec<TokenStream>,
    (String, TokenStream),
) {
    let base = &u.py;
    let base_name = LitStr::new(&format!("{}Base", base), base.span());
    let handle_field = &u.accessor;
    let handle_ty = u.handle.field_ty(&u.native);

    // Base struct: data/model + the native handle.
    let base_struct = quote! {
        #[::pyo3_stub_gen::derive::gen_stub_pyclass]
        #[::pyo3::pyclass(subclass, frozen, name = #base_name)]
        pub struct #base {
            pub(crate) data: ::std::sync::Arc<crate::ir_core::ModelData>,
            pub(crate) model: ::pyo3::Py<crate::model::Model>,
            pub(crate) #handle_field: #handle_ty,
        }
    };

    // Subclasses + their field getters.
    let mut subclass_defs = Vec::new();
    let mut register_calls = vec![quote!(m.add_class::<#base>()?;)];
    let mut dispatch_arms = Vec::new();
    let mut ref_members: Vec<TokenStream> = Vec::new();

    for v in &u.variants {
        let sub = &v.subclass;
        let variant = &v.native_variant;
        let native = &u.native;
        register_calls.push(quote!(m.add_class::<#sub>()?;));
        ref_members.push(quote!(#sub));
        dispatch_arms.push(quote! {
            #native::#variant { .. } => ::pyo3::Bound::new(
                py,
                ::pyo3::PyClassInitializer::from(base).add_subclass(#sub),
            )?.into_super().unbind()
        });

        let payload = payloads.iter().find(|p| p.name == v.subclass);
        let getters = emit_subclass_getters(u, v, payload, reg);
        subclass_defs.push(quote! {
            #[::pyo3_stub_gen::derive::gen_stub_pyclass]
            #[::pyo3::pyclass(extends = #base, frozen)]
            pub struct #sub;
            #[::pyo3_stub_gen::derive::gen_stub_pymethods]
            #[::pyo3::pymethods]
            impl #sub {
                #(#getters)*
            }
        });
    }
    if u.include_base {
        ref_members.push(quote!(#base));
    }

    let methods = emit_union_methods(u, reg);
    let identity_methods = emit_union_identity(u);
    let repr_method = emit_union_repr(u);
    let kind_helper = emit_union_kind_helper(u);

    // Dispatch + register (base + subclasses + the runtime union object).
    let native = &u.native;
    let alias = &u.alias;
    let ref_ty = format_ident!("{}Ref", base);
    let dispatch = quote! {
        #[::pyo3_stub_gen::derive::gen_stub_pymethods]
        #[::pyo3::pymethods]
        impl #base {
            #(#methods)*
            #identity_methods
            #repr_method
        }

        #kind_helper

        impl #base {
            /// Box `base` as the concrete subclass matching `disc`'s variant.
            pub(crate) fn dispatch(
                base: Self,
                py: ::pyo3::Python<'_>,
                disc: &#native,
            ) -> ::pyo3::PyResult<::pyo3::Py<Self>> {
                Ok(match disc {
                    #(#dispatch_arms,)*
                })
            }

            /// Register the base + every subclass, then add the runtime union.
            pub(crate) fn register(
                m: &::pyo3::Bound<'_, ::pyo3::types::PyModule>,
            ) -> ::pyo3::PyResult<()> {
                use ::pyo3::prelude::*;
                #(#register_calls)*
                let __classes: ::std::vec::Vec<::pyo3::Bound<'_, ::pyo3::PyAny>> =
                    ::std::vec![ #( m.py().get_type::<#ref_members>().into_any() ),* ];
                let mut __it = __classes.into_iter();
                let mut __acc = __it.next().expect("a union has at least one member");
                for __c in __it {
                    __acc = __acc.call_method1("__or__", (__c,))?;
                }
                m.add(#alias, __acc)?;
                Ok(())
            }
        }
    };

    // The `*Ref` return newtype: runtime object is the concrete subclass; its
    // stub type renders as the union alias.
    let ref_newtype = quote! {
        pub struct #ref_ty(pub ::pyo3::Py<::pyo3::PyAny>);
        impl<'py> ::pyo3::IntoPyObject<'py> for #ref_ty {
            type Target = ::pyo3::PyAny;
            type Output = ::pyo3::Bound<'py, ::pyo3::PyAny>;
            type Error = ::std::convert::Infallible;
            fn into_pyobject(self, py: ::pyo3::Python<'py>) -> ::std::result::Result<Self::Output, Self::Error> {
                ::std::result::Result::Ok(self.0.into_bound(py))
            }
        }
        impl ::pyo3_stub_gen::PyStubType for #ref_ty {
            fn type_output() -> ::pyo3_stub_gen::TypeInfo {
                ::pyo3_stub_gen::TypeInfo::unqualified(#alias)
            }
        }
        impl #ref_ty {
            /// The `Sub1 | Sub2 | …` expansion used as the `.pyi` alias RHS.
            pub fn union_typeinfo() -> ::pyo3_stub_gen::TypeInfo {
                let parts: ::std::vec::Vec<::pyo3_stub_gen::TypeInfo> = ::std::vec![
                    #( <#ref_members as ::pyo3_stub_gen::PyStubType>::type_output() ),*
                ];
                parts.into_iter().reduce(|a, b| a | b).expect("a union has at least one member")
            }
        }
    };

    // The `*_ref` builder wraps the concrete subclass (built by `build_*`) as the
    // `*Ref` newtype. `build_*` is generated here (default dispatch) unless the
    // union is `custom_build` (a per-hierarchy quirk lives in `crate::sem::hand`,
    // e.g. the unknown `Type`).
    let snake_name = snake(&base.to_string());
    let ref_fn = format_ident!("{}_ref", snake_name);
    let build_fn = format_ident!("build_{}", snake_name);
    let native = &u.native;
    // The native TYPE comes from the union's parsed `native` path (Fix A: no
    // literal `fpp_analysis::semantics::*` — moving that module is a pure regen).
    // The four handles differ only in STORAGE STRATEGY: Arc-wrapped, owned, or
    // borrowed.
    let (ref_arg, build_arg) = match u.handle {
        Handle::ArcType => (quote!(ty: ::std::sync::Arc<#native>), quote!(ty)),
        Handle::Value => (quote!(v: &#native), quote!(v)),
        Handle::Symbol => (quote!(s: #native), quote!(s)),
        Handle::Clone => (quote!(n: &#native), quote!(n)),
    };
    let ref_builder = quote! {
        pub fn #ref_fn(
            model: &::pyo3::Py<crate::model::Model>,
            py: ::pyo3::Python<'_>,
            #ref_arg,
        ) -> ::pyo3::PyResult<#ref_ty> {
            Ok(#ref_ty(crate::sem::#build_fn(model, py, #build_arg)?.into_any()))
        }
    };

    // Default (quirk-free) `build_*`: construct the base handle then dispatch.
    let build_default = if u.custom_build {
        quote!()
    } else {
        let (b_arg, owned, disc) = match u.handle {
            Handle::ArcType => (
                quote!(ty: ::std::sync::Arc<#native>),
                quote!(ty.clone()),
                quote!(&ty),
            ),
            Handle::Value => (quote!(v: &#native), quote!(v.clone()), quote!(v)),
            Handle::Symbol => (quote!(s: #native), quote!(s.clone()), quote!(&s)),
            Handle::Clone => (quote!(n: &#native), quote!(n.clone()), quote!(n)),
        };
        quote! {
            /// Build (dispatching to the concrete subclass) the wrapper for a
            /// native value.
            pub fn #build_fn(
                model: &::pyo3::Py<crate::model::Model>,
                py: ::pyo3::Python<'_>,
                #b_arg,
            ) -> ::pyo3::PyResult<::pyo3::Py<#base>> {
                let base = #base {
                    data: model.borrow(py).data.clone(),
                    model: model.clone_ref(py),
                    #handle_field: #owned,
                };
                #base::dispatch(base, py, #disc)
            }
        }
    };

    let alias_str = u.alias.value();
    let alias_entry = quote!(#ref_ty::union_typeinfo().name);

    (
        quote! { #base_struct #(#subclass_defs)* #dispatch #ref_newtype #ref_builder #build_default },
        vec![quote!(#base::register(m)?;)],
        Vec::new(),
        (alias_str, alias_entry),
    )
}

/// Emit a standalone `entity` item. Returns `(definition, register_call)`.
fn emit_entity(e: &EntityDecl, reg: &UnionReg) -> (TokenStream, TokenStream) {
    match &e.handle {
        EntityHandle::Clone { field } => emit_entity_clone(e, field, reg),
    }
}

/// Emit the `__eq__`/`__hash__` methods for a clone-handle entity's `identity`
/// directive (empty when default identity). `field` is the stored native handle.
fn emit_entity_identity(e: &EntityDecl, field: &Ident) -> Vec<TokenStream> {
    let py = &e.py;
    match &e.identity {
        EntityIdentity::None => Vec::new(),
        EntityIdentity::Node => vec![quote! {
            fn __eq__(&self, other: &::pyo3::Bound<'_, ::pyo3::PyAny>) -> bool {
                match other.cast::<#py>() {
                    ::std::result::Result::Ok(o) => self.#field == o.borrow().#field,
                    ::std::result::Result::Err(_) => false,
                }
            }
            fn __hash__(&self) -> u64 {
                self.data.ids.get(&self.#field.node()).copied().unwrap_or(0) as u64
            }
        }],
        EntityIdentity::QualifiedName(method) => vec![quote! {
            fn __eq__(&self, other: &::pyo3::Bound<'_, ::pyo3::PyAny>) -> bool {
                match other.cast::<#py>() {
                    ::std::result::Result::Ok(o) => {
                        self.#field.#method() == o.borrow().#field.#method()
                    }
                    ::std::result::Result::Err(_) => false,
                }
            }
            fn __hash__(&self) -> u64 {
                use ::std::hash::{Hash as _, Hasher as _};
                let mut __h = ::std::collections::hash_map::DefaultHasher::new();
                self.#field.#method().hash(&mut __h);
                __h.finish()
            }
        }],
        EntityIdentity::RawHandle => vec![quote! {
            fn __eq__(&self, other: &::pyo3::Bound<'_, ::pyo3::PyAny>) -> bool {
                match other.cast::<#py>() {
                    ::std::result::Result::Ok(o) => self.#field == o.borrow().#field,
                    ::std::result::Result::Err(_) => false,
                }
            }
            fn __hash__(&self) -> u64 {
                use ::std::hash::{Hash as _, Hasher as _};
                let mut __h = ::std::collections::hash_map::DefaultHasher::new();
                self.#field.hash(&mut __h);
                __h.finish()
            }
        }],
    }
}

/// Emit a standalone `clone`-handle entity: the pyclass struct, its `build`
/// constructor, and the getter block. Returns `(definition, register_call)`.
fn emit_entity_clone(e: &EntityDecl, field: &Ident, reg: &UnionReg) -> (TokenStream, TokenStream) {
    let py = &e.py;
    let native = &e.native;
    let model = quote!(&self.model);
    let data = quote!(self.data);

    let extra_names: Vec<&Ident> = e.extras.iter().map(|f| &f.name).collect();
    let extra_tys: Vec<TokenStream> = e.extras.iter().map(|f| f.shape.ty(reg)).collect();
    let extra_decls = e.extras.iter().map(|f| {
        let n = &f.name;
        let t = f.shape.ty(reg);
        quote!(pub(crate) #n: #t)
    });

    let mut getters: Vec<TokenStream> = Vec::new();

    // Build-time extras: plain stored scalars, read directly off `self`.
    for f in &e.extras {
        if matches!(f.shape, Shape::Skip) {
            continue;
        }
        let n = &f.name;
        let expr = f.shape.expr(&quote!((&self.#n)), &model, &data, reg);
        getters.push(getter_tokens(&Getter::field(
            n.clone(),
            f.shape.ty(reg),
            f.shape.needs_py(),
            quote!(Ok(#expr)),
        )));
    }

    // Native fields: projected through the stored handle.
    for f in &e.fields {
        if matches!(f.shape, Shape::Skip) {
            continue;
        }
        let n = &f.name;
        let access = quote!((&self.#field.#n));
        let expr = f.shape.expr(&access, &model, &data, reg);
        getters.push(getter_tokens(&Getter::field(
            n.clone(),
            f.shape.ty(reg),
            f.shape.needs_py(),
            quote!(Ok(#expr)),
        )));
    }

    // Methods: `&self` accessors (or associated fns over `&Native`).
    let analysis_access = quote!(&self.data.analysis);
    for m in &e.methods {
        if matches!(m.shape, Shape::Skip) {
            continue;
        }
        let mname = &m.name;
        let (extra, call_args, is_method) = method_arg_parts(&m.params, &analysis_access);
        let call = if m.assoc {
            quote!(#native::#mname(&self.#field #(, #call_args)*))
        } else {
            quote!(self.#field.#mname(#(#call_args),*))
        };
        let expr = m.shape.expr(&quote!((&__r)), &model, &data, reg);
        // The native call may read the retained compiler context, so it runs
        // under `run_ref` (see `emit_union_methods`).
        getters.push(getter_tokens(&Getter {
            name: mname.clone(),
            shape_ty: m.shape.ty(reg),
            needs_py: m.shape.needs_py(),
            body: quote! {
                ::fpp_core::run_ref(&#data.ctx, || {
                    let __r = #call;
                    Ok(#expr)
                })
            },
            via_super: false,
            extra_params: extra,
            is_method,
        }));
    }

    // `__eq__`/`__hash__` from the `identity` directive (clone-handle entities).
    getters.extend(emit_entity_identity(e, field));

    // A default `__repr__` (`<PyName>`). Clone-handle entities never emit their
    // own repr elsewhere, so this is always safe to add.
    let repr_lit = format!("<{py}>");
    getters.push(quote! {
        fn __repr__(&self) -> ::std::string::String {
            ::std::string::String::from(#repr_lit)
        }
    });

    let def = quote! {
        #[::pyo3_stub_gen::derive::gen_stub_pyclass]
        #[::pyo3::pyclass(frozen)]
        pub struct #py {
            pub(crate) data: ::std::sync::Arc<crate::ir_core::ModelData>,
            pub(crate) model: ::pyo3::Py<crate::model::Model>,
            pub(crate) #field: #native,
            #(#extra_decls,)*
        }

        impl #py {
            /// Build a `Py`-boxed wrapper from a native value (+ build-time extras).
            #[allow(clippy::too_many_arguments)]
            pub(crate) fn build(
                model: &::pyo3::Py<crate::model::Model>,
                py: ::pyo3::Python<'_>,
                #field: #native,
                #(#extra_names: #extra_tys,)*
            ) -> ::pyo3::PyResult<::pyo3::Py<Self>> {
                ::pyo3::Py::new(py, #py {
                    data: model.borrow(py).data.clone(),
                    model: model.clone_ref(py),
                    #field,
                    #(#extra_names,)*
                })
            }
        }

        #[::pyo3_stub_gen::derive::gen_stub_pymethods]
        #[::pyo3::pymethods]
        impl #py {
            #(#getters)*
        }
    };
    (def, quote!(m.add_class::<#py>()?;))
}

/// Emit a leaf-enum mirror: the fieldless `#[pyclass(eq, eq_int)]` Python enum +
/// a `From<&native>` mapping each native variant onto it. Returns
/// `(definition, register_call)`.
fn emit_leaf_enum(e: &LeafEnumDecl) -> (TokenStream, TokenStream) {
    let py = &e.py;
    let native = &e.native;
    let variant_idents: Vec<&Ident> = e.variants.iter().map(|(v, _)| v).collect();
    let from_arms = e.variants.iter().map(|(v, pat)| {
        let lhs = match pat {
            LeafPattern::Unit => quote!(#native::#v),
            LeafPattern::Tuple => quote!(#native::#v(..)),
            LeafPattern::Struct => quote!(#native::#v { .. }),
        };
        quote!(#lhs => #py::#v)
    });
    let def = quote! {
        #[::pyo3_stub_gen::derive::gen_stub_pyclass_enum]
        #[::pyo3::pyclass(eq, eq_int, frozen, hash, skip_from_py_object)]
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        pub enum #py {
            #(#variant_idents),*
        }

        impl ::std::convert::From<&#native> for #py {
            fn from(__v: &#native) -> Self {
                match __v {
                    #(#from_arms),*
                }
            }
        }
    };
    (def, quote!(m.add_class::<#py>()?;))
}

/// Emit the root `Analysis` wrapper: a `#[pyclass(frozen)]` holding `{data, model}`,
/// one `#[getter]` per `fields {}` entry (reading `self.data.analysis.<name>`), one
/// getter/method per `methods {}` entry (calling `self.data.analysis.<name>(…)`), a
/// `build_analysis` constructor, and the register call. Returns
/// `(definition, register_call)`.
fn emit_analysis(a: &AnalysisDecl, reg: &UnionReg) -> (TokenStream, TokenStream) {
    let native = &a.native;
    let model = quote!(&self.model);
    let data = quote!(self.data);
    let analysis_access = quote!(&self.data.analysis);
    let mut getters: Vec<TokenStream> = Vec::new();

    // Fields: read `self.data.analysis.<name>` directly.
    for f in &a.fields {
        if matches!(f.shape, Shape::Skip) {
            continue;
        }
        let n = &f.name;
        let expr = f
            .shape
            .expr(&quote!((&self.data.analysis.#n)), &model, &data, reg);
        getters.push(getter_tokens(&Getter::field(
            n.clone(),
            f.shape.ty(reg),
            f.shape.needs_py(),
            quote!(Ok(#expr)),
        )));
    }

    // Methods: call `self.data.analysis.<name>(<args>)`.
    for m in &a.methods {
        if matches!(m.shape, Shape::Skip) {
            continue;
        }
        let mname = &m.name;
        let (extra, call_args, is_method) = method_arg_parts(&m.params, &analysis_access);
        let call = if m.assoc {
            quote!(#native::#mname(&self.data.analysis #(, #call_args)*))
        } else {
            quote!(self.data.analysis.#mname(#(#call_args),*))
        };
        let expr = m.shape.expr(&quote!((&__r)), &model, &data, reg);
        // The native call may read the retained compiler context, so it runs
        // under `run_ref` (see `emit_union_methods`).
        getters.push(getter_tokens(&Getter {
            name: mname.clone(),
            shape_ty: m.shape.ty(reg),
            needs_py: m.shape.needs_py(),
            body: quote! {
                ::fpp_core::run_ref(&#data.ctx, || {
                    let __r = #call;
                    Ok(#expr)
                })
            },
            via_super: false,
            extra_params: extra,
            is_method,
        }));
    }

    let def = quote! {
        #[::pyo3_stub_gen::derive::gen_stub_pyclass]
        #[::pyo3::pyclass(frozen)]
        pub struct Analysis {
            pub(crate) data: ::std::sync::Arc<crate::ir_core::ModelData>,
            pub(crate) model: ::pyo3::Py<crate::model::Model>,
        }

        /// Build the `Analysis` root wrapper for a model.
        pub fn build_analysis(
            model: &::pyo3::Py<crate::model::Model>,
            py: ::pyo3::Python<'_>,
        ) -> ::pyo3::PyResult<::pyo3::Py<Analysis>> {
            ::pyo3::Py::new(
                py,
                Analysis {
                    data: model.borrow(py).data.clone(),
                    model: model.clone_ref(py),
                },
            )
        }

        #[::pyo3_stub_gen::derive::gen_stub_pymethods]
        #[::pyo3::pymethods]
        impl Analysis {
            #(#getters)*
        }
    };
    (def, quote!(m.add_class::<Analysis>()?;))
}

pub fn expand(input: TokenStream) -> TokenStream {
    let dsl = match syn::parse2::<Dsl>(input) {
        Ok(d) => d,
        Err(e) => return e.to_compile_error(),
    };

    // The union registry: every declared union keyed by its Python name, so a
    // `union(<Name>)`/`rewrap(<Name>::V)` shape resolves its native path + handle.
    // Built purely from the declared unions — every referenced union is locally
    // declared in this single invocation.
    let union_reg: UnionReg = dsl
        .unions
        .iter()
        .map(|u| {
            let subclasses = u
                .variants
                .iter()
                .map(|v| (v.native_variant.to_string(), v.subclass.clone()))
                .collect();
            (
                u.py.to_string(),
                UnionInfo {
                    native: u.native.clone(),
                    handle: u.handle,
                    subclasses,
                },
            )
        })
        .collect();

    let mut defs = Vec::new();
    let mut register_calls = Vec::new();
    let mut alias_names = Vec::new();
    let mut alias_exprs = Vec::new();

    for u in &dsl.unions {
        let (def, reg, _extra, (alias_name, alias_expr)) = emit_union(u, &dsl.payloads, &union_reg);
        defs.push(def);
        register_calls.extend(reg);
        alias_names.push(alias_name);
        alias_exprs.push(alias_expr);
    }

    for e in &dsl.entities {
        let (def, reg) = emit_entity(e, &union_reg);
        defs.push(def);
        register_calls.push(reg);
    }

    for e in &dsl.leaf_enums {
        let (def, reg) = emit_leaf_enum(e);
        defs.push(def);
        register_calls.push(reg);
    }

    for a in &dsl.analysis {
        let (def, reg) = emit_analysis(a, &union_reg);
        defs.push(def);
        register_calls.push(reg);
    }

    quote! {
        // The single load-bearing `fpp_analysis` trait dependency of the generated
        // code: the `.node()` accessor (called by the `loc_from_node` getter and by
        // the `node` identity `__hash__`, on unions/entities and the symbol-keyed
        // scaffolding) is a `SymbolInterface` trait method, so the trait must be in
        // scope for method resolution. The method NAME `node` is not carried as a
        // DSL payload: it is a stable trait method, and parameterizing it would not
        // remove this import (a trait method needs its trait imported regardless of
        // name). A rename of the trait or of `node` is a deliberate `fpp_analysis`
        // change that updates this one line + the `.node()` call sites together.
        use fpp_analysis::semantics::SymbolInterface as _;
        use ::pyo3::prelude::*;

        #(#defs)*

        /// Register every generated semantic pyclass with the module.
        pub fn register(m: &::pyo3::Bound<'_, ::pyo3::types::PyModule>) -> ::pyo3::PyResult<()> {
            #(#register_calls)*
            Ok(())
        }

        /// `(alias name, `Sub1 | Sub2 | …` RHS)` for every generated closed union.
        pub fn union_aliases() -> ::std::vec::Vec<(&'static str, ::std::string::String)> {
            ::std::vec![ #( (#alias_names, #alias_exprs) ),* ]
        }
    }
}
