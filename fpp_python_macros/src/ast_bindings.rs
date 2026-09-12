//! The `fpp_ast_bindings!` function-like macro: expands a declarative mirror of
//! the `fpp_ast` grammar (emitted by the `bindgen` binary into
//! `fpp_python/src/ast/defs.rs`) into the PyO3 AST-node wrappers, the recording
//! walk, and the typed `visit_*` methods of `crate::visitor::NodeVisitor`.
//!
//! The DSL is parsed into a `Registry`/`Shape`/`Card` model; `emit_walk`/`emit_py`
//! emit tokens parameterized only by `&Registry`. The macro never reads `fpp_ast`
//! source — only its DSL tokens.
//!
//! A field's shape is a pure function of its DSL type expression + the block's
//! category sets: the `str(<field>)` form is a collapsed string leaf (its named
//! sub-field cloned); `String`/`bool`/`Span` are builtin scalars; a name in
//! `leaves {…}` is a rendered leaf; a `kind` name is an inline kind-enum; a
//! `node`/`union` name is a child; anything else is opaque.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use syn::parse::{Parse, ParseStream};
use syn::{Ident, Token, braced, bracketed, parenthesized};

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Card {
    One,
    Opt,
    Vec,
    OptVec,
}

#[derive(Clone)]
enum Shape {
    Str,
    Bool,
    Leaf(String),
    LeafOpt(String),
    /// A collapsed string leaf: the field's value is a named scalar sub-field
    /// cloned to a `str` (e.g. `str(data)` -> `<field>.data.clone()`). The
    /// accessor rides in the DSL so the macro never bakes in a field path.
    StrLeaf(String),
    StrLeafOpt(String),
    Skip,
    Child(Card, String),
    Kind(String),
}

struct FieldDef {
    name: String,
    shape: Shape,
}

struct StructDef {
    #[allow(dead_code)]
    name: String,
    fields: Vec<FieldDef>,
}

struct UnionDef {
    #[allow(dead_code)]
    name: String,
    variants: Vec<(String, String)>, // (variant ident, inner node type)
}

enum KindField {
    Unnamed(Shape),
    Named(Vec<FieldDef>),
    Unit,
}

struct KindVariant {
    name: String,
    field: KindField,
}

struct KindDef {
    #[allow(dead_code)]
    name: String,
    variants: Vec<KindVariant>,
}

/// Whether a leaf-enum variant carries a payload (dropped in the Python mirror,
/// but the match arm must still bind/ignore it).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Payload {
    Unit,
    Tuple,
    Struct,
}

struct LeafVariant {
    name: String,
    payload: Payload,
}

/// A leaf enum (e.g. `IntegerKind`) rendered as a fieldless `#[pyclass(eq, eq_int)]`
/// mirror — NOT an `enum.Enum` subclass — plus `.name`/`.value` getters and a
/// `From<&fpp_ast::X>` conversion.
struct LeafEnumDef {
    #[allow(dead_code)]
    name: String,
    variants: Vec<LeafVariant>,
}

/// The translation-unit root: the container type the walk enters, the field
/// access reaching its member `Vec`, and the member union walked per element.
struct RootDef {
    container: String,
    field: syn::Member,
    member_union: String,
}

#[derive(Default)]
struct Registry {
    node_structs: BTreeMap<String, StructDef>,
    unions: BTreeMap<String, UnionDef>,
    kinds: BTreeMap<String, KindDef>,
    leaf_enums: BTreeMap<String, LeafEnumDef>,
    is_node: BTreeSet<String>,
    is_union: BTreeSet<String>,
    shadowed: BTreeSet<String>,
    root: Option<RootDef>,
}

// ---------------------------------------------------------------------------
// DSL parsing
// ---------------------------------------------------------------------------

/// A `[T]?` / `[T]` / `T?` / `T` type expression, or the collapsed string-leaf
/// form `str(<field>)` / `str(<field>)?`. `str_accessor` is `Some` for the
/// leaf form, carrying the scalar sub-field to read (`ident` is then the `str`
/// marker token and unused).
struct TypeExpr {
    card: Card,
    ident: Ident,
    str_accessor: Option<Ident>,
}

impl Parse for TypeExpr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(syn::token::Bracket) {
            let content;
            bracketed!(content in input);
            let ident: Ident = content.parse()?;
            let card = if input.peek(Token![?]) {
                input.parse::<Token![?]>()?;
                Card::OptVec
            } else {
                Card::Vec
            };
            Ok(TypeExpr {
                card,
                ident,
                str_accessor: None,
            })
        } else {
            let ident: Ident = input.parse()?;
            // `str(<field>)` — a collapsed string leaf reading the named scalar
            // sub-field (e.g. `str(data)`), with an optional trailing `?`.
            if ident == "str" && input.peek(syn::token::Paren) {
                let content;
                parenthesized!(content in input);
                let accessor: Ident = content.parse()?;
                let card = if input.peek(Token![?]) {
                    input.parse::<Token![?]>()?;
                    Card::Opt
                } else {
                    Card::One
                };
                return Ok(TypeExpr {
                    card,
                    ident,
                    str_accessor: Some(accessor),
                });
            }
            let card = if input.peek(Token![?]) {
                input.parse::<Token![?]>()?;
                Card::Opt
            } else {
                Card::One
            };
            Ok(TypeExpr {
                card,
                ident,
                str_accessor: None,
            })
        }
    }
}

struct FieldDecl {
    name: Ident,
    ty: TypeExpr,
}

impl Parse for FieldDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty: TypeExpr = input.parse()?;
        Ok(FieldDecl { name, ty })
    }
}

/// `Variant(Inner)` or bare `Variant` (≡ `Variant(Variant)`).
struct UnionVariantDecl {
    variant: Ident,
    inner: Ident,
}

impl Parse for UnionVariantDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let variant: Ident = input.parse()?;
        let inner = if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            content.parse()?
        } else {
            variant.clone()
        };
        Ok(UnionVariantDecl { variant, inner })
    }
}

/// `Variant` (unit) | `Variant(TypeExpr)` (tuple) | `Variant { f: TypeExpr, … }`.
struct KindVariantDecl {
    name: Ident,
    field: KindFieldDecl,
}

enum KindFieldDecl {
    Unit,
    Unnamed(TypeExpr),
    Named(Vec<FieldDecl>),
}

impl Parse for KindVariantDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let field = if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            KindFieldDecl::Unnamed(content.parse()?)
        } else if input.peek(syn::token::Brace) {
            let content;
            braced!(content in input);
            let fields = content.parse_terminated(FieldDecl::parse, Token![,])?;
            KindFieldDecl::Named(fields.into_iter().collect())
        } else {
            KindFieldDecl::Unit
        };
        Ok(KindVariantDecl { name, field })
    }
}

/// `Variant` (unit) | `Variant(_)` (tuple payload) | `Variant{_}` (struct payload).
struct LeafVariantDecl {
    name: Ident,
    payload: Payload,
}

impl Parse for LeafVariantDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let payload = if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            let _ = content.parse::<proc_macro2::TokenStream>();
            Payload::Tuple
        } else if input.peek(syn::token::Brace) {
            let content;
            braced!(content in input);
            let _ = content.parse::<proc_macro2::TokenStream>();
            Payload::Struct
        } else {
            Payload::Unit
        };
        Ok(LeafVariantDecl { name, payload })
    }
}

/// `Name { V1, V2(_), … }` — a leaf enum with its variants.
struct LeafEnumDecl {
    name: Ident,
    variants: Vec<LeafVariantDecl>,
}

impl Parse for LeafEnumDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let content;
        braced!(content in input);
        let variants = content.parse_terminated(LeafVariantDecl::parse, Token![,])?;
        Ok(LeafEnumDecl {
            name,
            variants: variants.into_iter().collect(),
        })
    }
}

/// `root <Container>(<field>, <MemberUnion>)` — the walk entry point. `<field>`
/// is the tuple index or field name reaching the member `Vec` (e.g. `0` for
/// `TransUnit(Vec<ModuleMember>)`).
struct RootDecl {
    container: Ident,
    field: syn::Member,
    member_union: Ident,
}

impl Parse for RootDecl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let container: Ident = input.parse()?;
        let content;
        parenthesized!(content in input);
        let field: syn::Member = content.parse()?;
        content.parse::<Token![,]>()?;
        let member_union: Ident = content.parse()?;
        Ok(RootDecl {
            container,
            field,
            member_union,
        })
    }
}

/// The whole `fpp_ast_bindings!` body.
struct Dsl {
    root: Option<RootDecl>,
    leaves: Vec<LeafEnumDecl>,
    shadowed: Vec<Ident>,
    nodes: Vec<(Ident, Vec<FieldDecl>)>,
    unions: Vec<(Ident, Vec<UnionVariantDecl>)>,
    kinds: Vec<(Ident, Vec<KindVariantDecl>)>,
}

fn parse_ident_list(input: ParseStream) -> syn::Result<Vec<Ident>> {
    let content;
    braced!(content in input);
    let items = content.parse_terminated(Ident::parse, Token![,])?;
    Ok(items.into_iter().collect())
}

impl Parse for Dsl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut dsl = Dsl {
            root: None,
            leaves: Vec::new(),
            shadowed: Vec::new(),
            nodes: Vec::new(),
            unions: Vec::new(),
            kinds: Vec::new(),
        };
        while !input.is_empty() {
            let kw: Ident = input.parse()?;
            match kw.to_string().as_str() {
                "root" => dsl.root = Some(input.parse()?),
                "leaves" => {
                    let content;
                    braced!(content in input);
                    let items = content.parse_terminated(LeafEnumDecl::parse, Token![,])?;
                    dsl.leaves = items.into_iter().collect();
                }
                "shadowed" => dsl.shadowed = parse_ident_list(input)?,
                "node" => {
                    let name: Ident = input.parse()?;
                    let content;
                    braced!(content in input);
                    let fields = content.parse_terminated(FieldDecl::parse, Token![,])?;
                    dsl.nodes.push((name, fields.into_iter().collect()));
                }
                "union" => {
                    let name: Ident = input.parse()?;
                    let content;
                    braced!(content in input);
                    let vs = content.parse_terminated(UnionVariantDecl::parse, Token![,])?;
                    dsl.unions.push((name, vs.into_iter().collect()));
                }
                "kind" => {
                    let name: Ident = input.parse()?;
                    let content;
                    braced!(content in input);
                    let vs = content.parse_terminated(KindVariantDecl::parse, Token![,])?;
                    dsl.kinds.push((name, vs.into_iter().collect()));
                }
                other => {
                    return Err(syn::Error::new(
                        kw.span(),
                        format!(
                            "unknown section `{other}` (expected root/leaves/shadowed/node/union/kind)"
                        ),
                    ));
                }
            }
        }
        Ok(dsl)
    }
}

// ---------------------------------------------------------------------------
// Registry building (classify by type name + category sets)
// ---------------------------------------------------------------------------

/// The shape of a field, from its cardinality + type name + the category sets.
/// String leaves arrive as the DSL's `str(<field>)` form (see [`TypeExpr`]) and
/// are resolved before this by-name step; a type name is a builtin scalar, a
/// rendered leaf, an inline kind, or a child.
fn classify(card: Card, name: &str, leaves: &BTreeSet<String>, kinds: &BTreeSet<String>) -> Shape {
    match name {
        "String" => Shape::Str,
        "bool" => Shape::Bool,
        "Span" => Shape::Skip,
        n if leaves.contains(n) => {
            if card == Card::Opt {
                Shape::LeafOpt(n.to_string())
            } else {
                Shape::Leaf(n.to_string())
            }
        }
        n if kinds.contains(n) => Shape::Kind(n.to_string()),
        n => Shape::Child(card, n.to_string()),
    }
}

fn build_registry(dsl: Dsl) -> Registry {
    let leaves: BTreeSet<String> = dsl.leaves.iter().map(|l| l.name.to_string()).collect();
    let kinds_set: BTreeSet<String> = dsl.kinds.iter().map(|(n, _)| n.to_string()).collect();

    let leaf_enums: BTreeMap<String, LeafEnumDef> = dsl
        .leaves
        .iter()
        .map(|l| {
            let name = l.name.to_string();
            let variants = l
                .variants
                .iter()
                .map(|v| LeafVariant {
                    name: v.name.to_string(),
                    payload: v.payload,
                })
                .collect();
            (name.clone(), LeafEnumDef { name, variants })
        })
        .collect();

    let mut reg = Registry {
        leaf_enums,
        is_node: dsl.nodes.iter().map(|(n, _)| n.to_string()).collect(),
        is_union: dsl.unions.iter().map(|(n, _)| n.to_string()).collect(),
        shadowed: dsl.shadowed.iter().map(|i| i.to_string()).collect(),
        root: dsl.root.map(|r| RootDef {
            container: r.container.to_string(),
            field: r.field,
            member_union: r.member_union.to_string(),
        }),
        ..Registry::default()
    };

    let field_shape = |ty: &TypeExpr| {
        if let Some(acc) = &ty.str_accessor {
            return if ty.card == Card::Opt {
                Shape::StrLeafOpt(acc.to_string())
            } else {
                Shape::StrLeaf(acc.to_string())
            };
        }
        classify(ty.card, &ty.ident.to_string(), &leaves, &kinds_set)
    };

    for (name, fields) in &dsl.nodes {
        let fields = fields
            .iter()
            .map(|f| FieldDef {
                name: f.name.to_string(),
                shape: field_shape(&f.ty),
            })
            .collect();
        let name = name.to_string();
        reg.node_structs
            .insert(name.clone(), StructDef { name, fields });
    }

    for (name, vs) in &dsl.unions {
        let variants = vs
            .iter()
            .map(|v| (v.variant.to_string(), v.inner.to_string()))
            .collect();
        let name = name.to_string();
        reg.unions.insert(name.clone(), UnionDef { name, variants });
    }

    for (name, vs) in &dsl.kinds {
        let variants = vs
            .iter()
            .map(|v| {
                let field = match &v.field {
                    KindFieldDecl::Unit => KindField::Unit,
                    KindFieldDecl::Unnamed(ty) => KindField::Unnamed(field_shape(ty)),
                    KindFieldDecl::Named(fields) => KindField::Named(
                        fields
                            .iter()
                            .map(|f| FieldDef {
                                name: f.name.to_string(),
                                shape: field_shape(&f.ty),
                            })
                            .collect(),
                    ),
                };
                KindVariant {
                    name: v.name.to_string(),
                    field,
                }
            })
            .collect();
        let name = name.to_string();
        reg.kinds.insert(name.clone(), KindDef { name, variants });
    }

    reg
}

// ---------------------------------------------------------------------------
// Naming helpers
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

fn kind_base(name: &str) -> String {
    name.strip_suffix("Kind").unwrap_or(name).to_string()
}

fn ast_ty(name: &str) -> proc_macro2::Ident {
    format_ident!("{}", name)
}

fn walk_fn_ident(name: &str) -> proc_macro2::Ident {
    format_ident!("walk_{}", snake(name))
}

fn walk_kind_fn_ident(name: &str) -> proc_macro2::Ident {
    format_ident!("walk_{}", snake(name))
}

fn node_kind_ident(name: &str) -> proc_macro2::Ident {
    format_ident!("{}", name)
}

fn build_kind_fn_ident(name: &str) -> proc_macro2::Ident {
    format_ident!("build_{}", snake(name))
}

fn kind_ref_ident(name: &str) -> proc_macro2::Ident {
    format_ident!("{}Ref", name)
}

fn kind_variant_wid(kind_name: &str, variant: &str) -> proc_macro2::Ident {
    format_ident!("{}{}", kind_base(kind_name), variant)
}

fn union_ref_ident(name: &str) -> proc_macro2::Ident {
    format_ident!("{}Ref", name)
}

fn union_node_members(name: &str, reg: &Registry) -> Vec<proc_macro2::Ident> {
    let mut set = BTreeSet::new();
    collect_union_members(name, reg, &mut set);
    set.into_iter().map(|n| format_ident!("{}", n)).collect()
}

fn collect_union_members(name: &str, reg: &Registry, out: &mut BTreeSet<String>) {
    if let Some(u) = reg.unions.get(name) {
        for (_variant, inner) in &u.variants {
            if reg.is_union.contains(inner) {
                collect_union_members(inner, reg, out);
            } else if reg.is_node.contains(inner) && !reg.shadowed.contains(inner) {
                out.insert(inner.clone());
            }
        }
    }
}

enum ChildConv {
    Node(proc_macro2::Ident),
    Union(proc_macro2::Ident),
    Opaque,
}

fn classify_child(ty: &str, reg: &Registry) -> ChildConv {
    if reg.shadowed.contains(ty) {
        ChildConv::Opaque
    } else if reg.is_union.contains(ty) {
        if union_node_members(ty, reg).is_empty() {
            ChildConv::Opaque
        } else {
            ChildConv::Union(union_ref_ident(ty))
        }
    } else if reg.is_node.contains(ty) {
        ChildConv::Node(format_ident!("{}", ty))
    } else {
        ChildConv::Opaque
    }
}

fn child_item_ty(conv: &ChildConv) -> TokenStream {
    match conv {
        ChildConv::Node(t) => quote!(Py<#t>),
        ChildConv::Union(r) => quote!(#r),
        ChildConv::Opaque => quote!(Py<AstNode>),
    }
}

fn child_build(conv: &ChildConv, model_tok: &TokenStream, id_tok: TokenStream) -> TokenStream {
    match conv {
        ChildConv::Node(t) => quote! {
            Model::build(#model_tok, py, #id_tok)?.into_bound(py).into_any().cast_into::<#t>()?.unbind()
        },
        ChildConv::Union(r) => quote! {
            #r(Model::build(#model_tok, py, #id_tok)?.into_any())
        },
        ChildConv::Opaque => quote! {
            Model::build(#model_tok, py, #id_tok)?
        },
    }
}

// ---------------------------------------------------------------------------
// Emit: the recording walk (NodeKind + walk_* fns)
// ---------------------------------------------------------------------------

fn is_walkable(shape: &Shape) -> bool {
    matches!(shape, Shape::Child(..) | Shape::Kind(_))
}

fn child_walk_call(ty: &str, val: TokenStream, reg: &Registry) -> TokenStream {
    if reg.is_node.contains(ty) || reg.is_union.contains(ty) {
        let f = walk_fn_ident(ty);
        quote!(__kids.push(#f(w, #val));)
    } else {
        quote!()
    }
}

fn walk_field_recurse(shape: &Shape, access: TokenStream, reg: &Registry) -> TokenStream {
    match shape {
        Shape::Child(card, ty) => match card {
            Card::One => child_walk_call(ty, quote!(&#access), reg),
            Card::Opt => {
                let c = child_walk_call(ty, quote!(v), reg);
                quote!(if let Some(v) = #access.as_ref() { #c })
            }
            Card::Vec => {
                let c = child_walk_call(ty, quote!(v), reg);
                quote!(for v in &#access { #c })
            }
            Card::OptVec => {
                let c = child_walk_call(ty, quote!(v), reg);
                quote!(if let Some(vs) = #access.as_ref() { for v in vs { #c } })
            }
        },
        Shape::Kind(k) => {
            let f = walk_kind_fn_ident(k);
            quote!(#f(w, &#access, &mut __kids);)
        }
        _ => quote!(),
    }
}

fn walk_kind_recurse(shape: &Shape, bind: TokenStream, reg: &Registry) -> TokenStream {
    match shape {
        Shape::Child(card, ty) => match card {
            Card::One => child_walk_call(ty, bind, reg),
            Card::Opt => {
                let c = child_walk_call(ty, quote!(v), reg);
                quote!(if let Some(v) = #bind.as_ref() { #c })
            }
            Card::Vec => {
                let c = child_walk_call(ty, quote!(v), reg);
                quote!(for v in #bind { #c })
            }
            Card::OptVec => {
                let c = child_walk_call(ty, quote!(v), reg);
                quote!(if let Some(vs) = #bind.as_ref() { for v in vs { #c } })
            }
        },
        Shape::Kind(k) => {
            let f = walk_kind_fn_ident(k);
            quote!(#f(w, #bind, __kids);)
        }
        _ => quote!(),
    }
}

fn emit_walk(reg: &Registry) -> TokenStream {
    let mut kind_variants = Vec::new();
    for name in reg.node_structs.keys() {
        let v = node_kind_ident(name);
        kind_variants.push(quote!(#v));
    }

    let mut fns = Vec::new();

    for (name, def) in &reg.node_structs {
        let fnid = walk_fn_ident(name);
        let ty = ast_ty(name);
        let kind = node_kind_ident(name);
        let mut recs = Vec::new();
        for f in &def.fields {
            let fname = format_ident!("{}", f.name);
            let rec = walk_field_recurse(&f.shape, quote!(__node.#fname), reg);
            if !rec.is_empty() {
                recs.push(rec);
            }
        }
        // Childless nodes get no child list at all (absent == empty).
        let body = if recs.is_empty() {
            quote!()
        } else {
            quote! {
                let mut __kids: Vec<Node> = Vec::new();
                #(#recs)*
                w.set_children(__nid, __kids);
            }
        };
        fns.push(quote! {
            pub fn #fnid(w: &mut Walker, __node: &fpp_ast::#ty) -> Node {
                let __nid = fpp_ast::AstNode::id(__node);
                if w.enter(__nid, NodeKind::#kind, __node as *const _ as *const ()) {
                    #body
                }
                __nid
            }
        });
    }

    for (name, def) in &reg.unions {
        let fnid = walk_fn_ident(name);
        let ty = ast_ty(name);
        let mut arms = Vec::new();
        for (variant, inner) in &def.variants {
            let vid = format_ident!("{}", variant);
            if reg.is_node.contains(inner) || reg.is_union.contains(inner) {
                let f = walk_fn_ident(inner);
                arms.push(quote!(fpp_ast::#ty::#vid(x) => #f(w, x),));
            } else {
                arms.push(quote!(fpp_ast::#ty::#vid(x) => fpp_ast::AstNode::id(x),));
            }
        }
        fns.push(quote! {
            fn #fnid(w: &mut Walker, m: &fpp_ast::#ty) -> Node {
                match m { #(#arms)* }
            }
        });
    }

    for (name, def) in &reg.kinds {
        let fnid = walk_kind_fn_ident(name);
        let ty = ast_ty(name);
        let mut arms = Vec::new();
        for v in &def.variants {
            let vid = format_ident!("{}", v.name);
            match &v.field {
                KindField::Unit => arms.push(quote!(fpp_ast::#ty::#vid => {})),
                KindField::Unnamed(sh) => {
                    if is_walkable(sh) {
                        let r = walk_kind_recurse(sh, quote!(f0), reg);
                        arms.push(quote!(fpp_ast::#ty::#vid(f0) => { #r }));
                    } else {
                        arms.push(quote!(fpp_ast::#ty::#vid(_) => {}));
                    }
                }
                KindField::Named(fields) => {
                    let mut binds = Vec::new();
                    let mut body = Vec::new();
                    for f in fields {
                        if is_walkable(&f.shape) {
                            let fname = format_ident!("{}", f.name);
                            binds.push(quote!(#fname));
                            body.push(walk_kind_recurse(&f.shape, quote!(#fname), reg));
                        }
                    }
                    arms.push(quote!(fpp_ast::#ty::#vid { #(#binds,)* .. } => { #(#body)* }));
                }
            }
        }
        fns.push(quote! {
            fn #fnid(w: &mut Walker, k: &fpp_ast::#ty, __kids: &mut Vec<Node>) {
                match k { #(#arms)* }
            }
        });
    }

    // The walk entry point, from the `root` directive (defaulting to the
    // `TransUnit(0, ModuleMember)` shape for a DSL that omits it).
    let (root_container, root_field, root_member) = match &reg.root {
        Some(r) => (
            ast_ty(&r.container),
            r.field.clone(),
            r.member_union.clone(),
        ),
        None => (
            ast_ty("TransUnit"),
            syn::Member::Unnamed(syn::Index::from(0)),
            "ModuleMember".to_string(),
        ),
    };
    let root_member_walk = walk_fn_ident(&root_member);

    quote! {
        /// Which `fpp_ast` node a recorded pointer points at (drives wrapper
        /// construction in `construct`).
        #[derive(Clone, Copy, Debug)]
        pub enum NodeKind {
            #(#kind_variants,)*
            Opaque,
        }

        /// Walk a translation unit's members, recording each node's side-table
        /// facts and returning the root node handles (in source order).
        pub fn walk_trans_unit(w: &mut Walker, tu: &fpp_ast::#root_container) -> Vec<Node> {
            tu.#root_field.iter().map(|m| #root_member_walk(w, m)).collect()
        }

        #(#fns)*
    }
}

// ---------------------------------------------------------------------------
// Emit: the PyO3 wrappers (AstNode + node/kind/union wrappers + construct/register)
// ---------------------------------------------------------------------------

fn stub_attrs(name: &str, reg: &Registry) -> (TokenStream, TokenStream) {
    if reg.shadowed.contains(name) {
        (quote!(), quote!())
    } else {
        (quote!(#[gen_stub_pyclass]), quote!(#[gen_stub_pymethods]))
    }
}

fn emit_visitor_method(py_name: &str, param_ty: &proc_macro2::Ident) -> TokenStream {
    let fn_id = format_ident!("visit_{}", snake(py_name));
    let py_method = format!("visit_{}", py_name);
    quote! {
        #[pyo3(name = #py_method)]
        fn #fn_id(slf: PyRef<'_, Self>, node: &Bound<'_, #param_ty>) -> PyResult<Py<PyAny>> {
            NodeVisitor::delegate_to_generic_visit(slf, node.as_any())
        }
    }
}

fn single_field_name(shape: &Shape) -> proc_macro2::Ident {
    match shape {
        Shape::Child(Card::Vec, _) | Shape::Child(Card::OptVec, _) => format_ident!("elements"),
        _ => format_ident!("value"),
    }
}

fn emit_py(reg: &Registry) -> TokenStream {
    let mut wrappers = Vec::new();
    let mut construct_arms = Vec::new();
    let mut register_calls = Vec::new();
    let mut visitor_methods = Vec::new();

    for (name, def) in &reg.node_structs {
        let wid = format_ident!("{}", name);
        let ty = ast_ty(name);
        let kind = node_kind_ident(name);
        let (gsc, gsm) = stub_attrs(name, reg);
        register_calls.push(quote!(m.add_class::<#wid>()?;));
        let param_ty = if reg.shadowed.contains(name) {
            format_ident!("AstNode")
        } else {
            wid.clone()
        };
        visitor_methods.push(emit_visitor_method(name, &param_ty));
        construct_arms.push(quote! {
            Some(NodeKind::#kind) => Bound::new(py, PyClassInitializer::from(AstNode { data: data.clone(), model: model.clone_ref(py), node }).add_subclass(#wid))?.into_super().unbind()
        });

        let mut getters = Vec::new();
        for f in &def.fields {
            if matches!(f.shape, Shape::Skip) {
                continue;
            }
            let fname = format_ident!("{}", f.name);
            getters.push(emit_getter(&ty, &fname, &f.shape, reg));
        }

        let repr = format!("<{} #{{}}>", name);
        wrappers.push(quote! {
            #gsc
            #[pyclass(extends = AstNode, frozen)]
            pub struct #wid;
            #gsm
            #[pymethods]
            impl #wid {
                #(#getters)*
                fn __repr__(self_: PyRef<'_, Self>) -> String {
                    let sup = self_.as_super();
                    format!(#repr, sup.data.id(sup.node))
                }
            }
        });
    }

    let mut kind_wrappers = Vec::new();
    for (name, def) in &reg.kinds {
        let kty = ast_ty(name);
        let buildfn = build_kind_fn_ident(name);
        let refty = kind_ref_ident(name);
        let mut build_arms = Vec::new();
        let mut variant_wids = Vec::new();
        for v in &def.variants {
            let wid = kind_variant_wid(name, &v.name);
            let vid = format_ident!("{}", v.name);
            variant_wids.push(wid.clone());
            register_calls.push(quote!(m.add_class::<#wid>()?;));
            match &v.field {
                KindField::Unit => {
                    kind_wrappers.push(quote! {
                        #[gen_stub_pyclass]
                        #[pyclass(frozen)] pub struct #wid {}
                    });
                    build_arms
                        .push(quote!(fpp_ast::#kty::#vid => Py::new(py, #wid {})?.into_any()));
                }
                KindField::Unnamed(sh) => {
                    if matches!(sh, Shape::Skip) {
                        kind_wrappers.push(
                            quote!(#[gen_stub_pyclass] #[pyclass(frozen)] pub struct #wid {}),
                        );
                        build_arms.push(
                            quote!(fpp_ast::#kty::#vid(_) => Py::new(py, #wid {})?.into_any()),
                        );
                    } else {
                        let fname = single_field_name(sh);
                        let (decl, ctor, getter, needs_model) =
                            kind_field_parts(&fname, sh, quote!(f0), reg);
                        let model_decl = if needs_model {
                            quote!(model: Py<Model>,)
                        } else {
                            quote!()
                        };
                        let model_init = if needs_model {
                            quote!(model: model.clone_ref(py),)
                        } else {
                            quote!()
                        };
                        kind_wrappers.push(quote! {
                            #[gen_stub_pyclass]
                            #[pyclass(frozen)] pub struct #wid { #model_decl #decl }
                            #[gen_stub_pymethods]
                            #[pymethods] impl #wid { #getter }
                        });
                        build_arms.push(quote! {
                            fpp_ast::#kty::#vid(f0) => Py::new(py, #wid { #model_init #ctor })?.into_any()
                        });
                    }
                }
                KindField::Named(fields) => {
                    let mut decls = Vec::new();
                    let mut binds = Vec::new();
                    let mut ctor_inits = Vec::new();
                    let mut getters = Vec::new();
                    let mut needs_model = false;
                    for f in fields {
                        if matches!(f.shape, Shape::Skip) {
                            continue;
                        }
                        let fname = format_ident!("{}", f.name);
                        binds.push(quote!(#fname));
                        let (decl, init, getter, nm) =
                            kind_field_parts(&fname, &f.shape, quote!(#fname), reg);
                        decls.push(decl);
                        ctor_inits.push(init);
                        getters.push(getter);
                        needs_model = needs_model || nm;
                    }
                    let model_decl = if needs_model {
                        quote!(model: Py<Model>,)
                    } else {
                        quote!()
                    };
                    let model_init = if needs_model {
                        quote!(model: model.clone_ref(py),)
                    } else {
                        quote!()
                    };
                    kind_wrappers.push(quote! {
                        #[gen_stub_pyclass]
                        #[pyclass(frozen)] pub struct #wid { #model_decl #(#decls,)* }
                        #[gen_stub_pymethods]
                        #[pymethods] impl #wid { #(#getters)* }
                    });
                    build_arms.push(quote! {
                        fpp_ast::#kty::#vid { #(#binds,)* .. } => Py::new(py, #wid { #model_init #(#ctor_inits,)* })?.into_any()
                    });
                }
            }
        }
        kind_wrappers.push(quote! {
            pub struct #refty(Py<PyAny>);
            pyo3_stub_gen::impl_stub_type!(#refty = #(#variant_wids)|*);
            impl<'py> IntoPyObject<'py> for #refty {
                type Target = PyAny;
                type Output = Bound<'py, PyAny>;
                type Error = std::convert::Infallible;
                fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
                    Ok(self.0.into_bound(py))
                }
            }
            pub fn #buildfn(model: &Py<Model>, py: Python<'_>, k: &fpp_ast::#kty) -> PyResult<#refty> {
                let _ = model;
                Ok(#refty(match k { #(#build_arms,)* }))
            }
        });
    }

    // `Opaque` is the `construct` fallback wrapper — a visitable node like any
    // other, so it gets a base method too (last, after the declared nodes).
    visitor_methods.push(emit_visitor_method("Opaque", &format_ident!("Opaque")));

    let mut union_wrappers = Vec::new();
    for name in reg.unions.keys() {
        let members = union_node_members(name, reg);
        if members.is_empty() {
            continue;
        }
        let refty = union_ref_ident(name);
        union_wrappers.push(quote! {
            pub struct #refty(Py<PyAny>);
            pyo3_stub_gen::impl_stub_type!(#refty = #(#members)|*);
            impl<'py> IntoPyObject<'py> for #refty {
                type Target = PyAny;
                type Output = Bound<'py, PyAny>;
                type Error = std::convert::Infallible;
                fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
                    Ok(self.0.into_bound(py))
                }
            }
        });
    }

    // Leaf enums: a fieldless `#[pyclass(eq, eq_int)]` mirror per leaf + `.name`/
    // `.value` getters + a `From<&fpp_ast::X>` that maps each native variant onto it
    // (payload, if any, is dropped — the Python mirror only carries the
    // discriminant). PyO3 gives a simple enum only `__repr__`/`__int__`/
    // `__richcmp__`/`__hash__`, so without the getters the member spelling would be
    // recoverable only by parsing `repr`. For `IntegerKind`/`FloatKind` that spelling
    // is the FPP (and F Prime C++) type name a generator needs directly; for the
    // keyword kinds it is the model's own name, and FPP's surface syntax differs
    // (`passive`, `activity high`) — which is what `leaf_enum_doc` tells callers.
    let mut leaf_defs = Vec::new();
    for (name, def) in &reg.leaf_enums {
        let ety = format_ident!("{}", name);
        let nat = ast_ty(name);
        let variant_ids: Vec<_> = def
            .variants
            .iter()
            .map(|v| format_ident!("{}", v.name))
            .collect();
        let variant_names: Vec<&str> = def.variants.iter().map(|v| v.name.as_str()).collect();
        let from_arms: Vec<_> = def
            .variants
            .iter()
            .map(|v| {
                let vid = format_ident!("{}", v.name);
                let pat = match v.payload {
                    Payload::Unit => quote!(fpp_ast::#nat::#vid),
                    Payload::Tuple => quote!(fpp_ast::#nat::#vid(..)),
                    Payload::Struct => quote!(fpp_ast::#nat::#vid { .. }),
                };
                quote!(#pat => #ety::#vid)
            })
            .collect();
        register_calls.push(quote!(m.add_class::<#ety>()?;));
        let doc = crate::leaf_enum_doc(name);
        leaf_defs.push(quote! {
            #[doc = #doc]
            #[gen_stub_pyclass_enum]
            #[pyclass(eq, eq_int, frozen, hash, skip_from_py_object)]
            // `Ord` is Rust-only (`#[pyclass(ord)]` stays off, so nothing appears in
            // the stub): it is what lets a map keyed by this mirror iterate in
            // native declaration order — see `sem_bindings::KeyOrder`.
            #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
            pub enum #ety { #(#variant_ids,)* }

            // A plain `impl`, not the `#[pymethods]` block below, so the getters
            // delegate to it rather than to each other through PyO3's rewriting.
            impl #ety {
                fn member_name(&self) -> &'static str {
                    match self { #(Self::#variant_ids => #variant_names,)* }
                }
            }

            #[gen_stub_pymethods]
            #[pymethods]
            impl #ety {
                /// Get enum variant name as a string
                #[getter]
                fn name(&self) -> &'static str { self.member_name() }
                /// The same string as `name` (`enum.StrEnum`).
                #[getter]
                fn value(&self) -> &'static str { self.member_name() }
            }

            impl ::std::convert::From<&fpp_ast::#nat> for #ety {
                fn from(x: &fpp_ast::#nat) -> Self {
                    match x { #(#from_arms,)* }
                }
            }
        });
    }

    quote! {
        /// Base class for every AST-node wrapper: holds the `Model` handle + the
        /// node, and the getters common to all nodes. Node wrappers are
        /// `#[pyclass(extends = AstNode)]` unit subclasses.
        #[gen_stub_pyclass]
        #[pyclass(subclass, frozen)]
        // `data` is the backing model data, captured once at construction (the same
        // `Arc` as `model.borrow(py).data`) and read directly by every getter —
        // mirroring the semantic-layer wrappers. `model` is kept only to build child
        // wrappers via the memoizing `Model::build`. All three are `pub(crate)` so
        // `crate::visitor` can traverse from a `&Bound<AstNode>` (the class is
        // `frozen`, so reading them needs no borrow flag).
        pub struct AstNode {
            pub(crate) data: ::std::sync::Arc<crate::ir_core::ModelData>,
            pub(crate) model: Py<Model>,
            pub(crate) node: Node,
        }

        #[gen_stub_pymethods]
        #[pymethods]
        impl AstNode {
            #[getter] fn node_id(&self) -> u32 { self.data.id(self.node) }
            #[getter] fn location(&self) -> Loc { self.data.loc(self.node) }
            #[getter] fn span(&self) -> crate::ir_core::Span {
                crate::ir_core::Span::new(self.data.clone(), self.data.span_of(self.node))
            }
            /// Whether this node belongs to a unit the caller asked about, rather
            /// than one passed to `analyze(imports=...)` to resolve references.
            #[getter] fn in_source(&self) -> bool { self.data.in_source(self.node) }
            /// This node's direct child nodes, in source order.
            #[getter] fn children(&self, py: Python<'_>) -> PyResult<Vec<Py<AstNode>>> {
                let kids: Vec<Node> = self.data.children(self.node).to_vec();
                kids.into_iter().map(|c| Model::build(&self.model, py, c)).collect()
            }
            #[getter] fn pre_annotation(&self) -> Vec<String> { self.data.pre_anno(self.node) }
            #[getter] fn post_annotation(&self) -> Vec<String> { self.data.post_anno(self.node) }
            /// The definition this use-site node resolves to (or None).
            #[getter] fn definition(&self, py: Python<'_>) -> PyResult<Option<crate::sem::SymbolRef>> {
                match self.data.use_def(self.node) {
                    Some(s) => Ok(Some(crate::sem::SymbolRef(crate::sem::build_symbol(&self.model, py, s.clone())?.into_any()))),
                    None => Ok(None),
                }
            }
            /// The resolved type of this node (or None).
            #[getter] fn resolved_type(&self, py: Python<'_>) -> PyResult<Option<crate::sem::TypeRef>> {
                match self.data.type_of(self.node) {
                    Some(ty) => Ok(Some(crate::sem::TypeRef(crate::sem::build_type(&self.model, py, ty)?.into_any()))),
                    None => Ok(None),
                }
            }
            /// The resolved (constant-folded) value of this node (or None).
            #[getter] fn resolved_value(&self, py: Python<'_>) -> PyResult<Option<crate::sem::ValueRef>> {
                match self.data.value_of(self.node) {
                    Some(v) => Ok(Some(crate::sem::ValueRef(crate::sem::build_value(&self.model, py, v)?.into_any()))),
                    None => Ok(None),
                }
            }
        }

        pub fn construct(model: &Py<Model>, py: Python<'_>, node: Node) -> PyResult<Py<AstNode>> {
            let data = model.borrow(py).data.clone();
            let tag = data.node_ptrs.get(&node).map(|nr| nr.tag);
            Ok(match tag {
                #(#construct_arms,)*
                _ => Bound::new(py, PyClassInitializer::from(AstNode { data: data.clone(), model: model.clone_ref(py), node }).add_subclass(Opaque))?.into_super().unbind(),
            })
        }

        #(#wrappers)*
        #(#kind_wrappers)*
        #(#union_wrappers)*
        #(#leaf_defs)*

        #[gen_stub_pymethods]
        #[pymethods]
        impl NodeVisitor {
            #(#visitor_methods)*
        }

        #[gen_stub_pyclass]
        #[pyclass(extends = AstNode, frozen)]
        pub struct Opaque;
        #[gen_stub_pymethods]
        #[pymethods]
        impl Opaque {
            #[getter] fn kind(&self) -> String { "Opaque".to_string() }
            fn __repr__(self_: PyRef<'_, Self>) -> String {
                let sup = self_.as_super();
                format!("<Opaque #{}>", sup.data.id(sup.node))
            }
        }

        pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
            m.add_class::<AstNode>()?;
            m.add_class::<Opaque>()?;
            #(#register_calls)*
            Ok(())
        }
    }
}

fn emit_getter(
    node_ty: &proc_macro2::Ident,
    fname: &proc_macro2::Ident,
    shape: &Shape,
    reg: &Registry,
) -> TokenStream {
    match shape {
        Shape::Str => quote! {
            #[getter] fn #fname(self_: PyRef<'_, Self>) -> String {
                let sup = self_.as_super();
                sup.data.node_as::<fpp_ast::#node_ty>(sup.node).#fname.clone()
            }
        },
        Shape::StrLeaf(acc) => {
            let acc = format_ident!("{}", acc);
            quote! {
                #[getter] fn #fname(self_: PyRef<'_, Self>) -> String {
                    let sup = self_.as_super();
                    sup.data.node_as::<fpp_ast::#node_ty>(sup.node).#fname.#acc.clone()
                }
            }
        }
        Shape::Leaf(l) => {
            let ety = format_ident!("{}", l);
            quote! {
                #[getter] fn #fname(self_: PyRef<'_, Self>) -> #ety {
                    let sup = self_.as_super();
                    #ety::from(&sup.data.node_as::<fpp_ast::#node_ty>(sup.node).#fname)
                }
            }
        }
        Shape::StrLeafOpt(acc) => {
            let acc = format_ident!("{}", acc);
            quote! {
                #[getter] fn #fname(self_: PyRef<'_, Self>) -> Option<String> {
                    let sup = self_.as_super();
                    sup.data.node_as::<fpp_ast::#node_ty>(sup.node).#fname.as_ref().map(|v| v.#acc.clone())
                }
            }
        }
        Shape::LeafOpt(l) => {
            let ety = format_ident!("{}", l);
            quote! {
                #[getter] fn #fname(self_: PyRef<'_, Self>) -> Option<#ety> {
                    let sup = self_.as_super();
                    sup.data.node_as::<fpp_ast::#node_ty>(sup.node).#fname.as_ref().map(|v| #ety::from(v))
                }
            }
        }
        Shape::Bool => quote! {
            #[getter] fn #fname(self_: PyRef<'_, Self>) -> bool {
                let sup = self_.as_super();
                sup.data.node_as::<fpp_ast::#node_ty>(sup.node).#fname
            }
        },
        Shape::Skip => quote!(),
        Shape::Child(card, ty) => {
            let conv = classify_child(ty, reg);
            let item_ty = child_item_ty(&conv);
            let model_tok = quote!(&sup.model);
            match card {
                Card::One => {
                    let build = child_build(&conv, &model_tok, quote!(child));
                    quote! {
                        #[getter] fn #fname(self_: PyRef<'_, Self>, py: Python<'_>) -> PyResult<#item_ty> {
                            let sup = self_.as_super();
                            let child = sup.data.node_as::<fpp_ast::#node_ty>(sup.node).#fname.id();
                            Ok(#build)
                        }
                    }
                }
                Card::Opt => {
                    let build = child_build(&conv, &model_tok, quote!(c));
                    quote! {
                        #[getter] fn #fname(self_: PyRef<'_, Self>, py: Python<'_>) -> PyResult<Option<#item_ty>> {
                            let sup = self_.as_super();
                            let child = sup.data.node_as::<fpp_ast::#node_ty>(sup.node).#fname.as_ref().map(|v| v.id());
                            match child { Some(c) => Ok(Some(#build)), None => Ok(None) }
                        }
                    }
                }
                Card::Vec => {
                    let build = child_build(&conv, &model_tok, quote!(*c));
                    quote! {
                        #[getter] fn #fname(self_: PyRef<'_, Self>, py: Python<'_>) -> PyResult<Vec<#item_ty>> {
                            let sup = self_.as_super();
                            let children: Vec<Node> = sup.data.node_as::<fpp_ast::#node_ty>(sup.node).#fname.iter().map(|v| v.id()).collect();
                            children.iter().map(|c| Ok(#build)).collect()
                        }
                    }
                }
                Card::OptVec => {
                    let build = child_build(&conv, &model_tok, quote!(*c));
                    quote! {
                        #[getter] fn #fname(self_: PyRef<'_, Self>, py: Python<'_>) -> PyResult<Option<Vec<#item_ty>>> {
                            let sup = self_.as_super();
                            let children: Option<Vec<Node>> = sup.data.node_as::<fpp_ast::#node_ty>(sup.node).#fname.as_ref().map(|vs| vs.iter().map(|v| v.id()).collect());
                            match children {
                                Some(cs) => Ok(Some(cs.iter().map(|c| Ok(#build)).collect::<PyResult<Vec<_>>>()?)),
                                None => Ok(None),
                            }
                        }
                    }
                }
            }
        }
        Shape::Kind(k) => {
            let buildfn = build_kind_fn_ident(k);
            let refty = kind_ref_ident(k);
            let _ = reg;
            quote! {
                #[getter] fn #fname(self_: PyRef<'_, Self>, py: Python<'_>) -> PyResult<#refty> {
                    let sup = self_.as_super();
                    let n = sup.data.node_as::<fpp_ast::#node_ty>(sup.node);
                    #buildfn(&sup.model, py, &n.#fname)
                }
            }
        }
    }
}

fn kind_field_parts(
    fname: &proc_macro2::Ident,
    shape: &Shape,
    bind: TokenStream,
    reg: &Registry,
) -> (TokenStream, TokenStream, TokenStream, bool) {
    match shape {
        Shape::Str => (
            quote!(#[pyo3(get)] #fname: String),
            quote!(#fname: #bind.clone()),
            quote!(),
            false,
        ),
        Shape::StrLeaf(acc) => {
            let acc = format_ident!("{}", acc);
            (
                quote!(#[pyo3(get)] #fname: String),
                quote!(#fname: #bind.#acc.clone()),
                quote!(),
                false,
            )
        }
        Shape::Leaf(l) => {
            let ety = format_ident!("{}", l);
            (
                quote!(#[pyo3(get)] #fname: #ety),
                quote!(#fname: #ety::from(#bind)),
                quote!(),
                false,
            )
        }
        Shape::StrLeafOpt(acc) => {
            let acc = format_ident!("{}", acc);
            (
                quote!(#[pyo3(get)] #fname: Option<String>),
                quote!(#fname: #bind.as_ref().map(|v| v.#acc.clone())),
                quote!(),
                false,
            )
        }
        Shape::LeafOpt(l) => {
            let ety = format_ident!("{}", l);
            (
                quote!(#[pyo3(get)] #fname: Option<#ety>),
                quote!(#fname: #bind.as_ref().map(|v| #ety::from(v))),
                quote!(),
                false,
            )
        }
        Shape::Bool => (
            quote!(#[pyo3(get)] #fname: bool),
            quote!(#fname: *#bind),
            quote!(),
            false,
        ),
        Shape::Child(Card::One, ty) => {
            let conv = classify_child(ty, reg);
            let item_ty = child_item_ty(&conv);
            let build = child_build(&conv, &quote!(&self.model), quote!(self.#fname));
            (
                quote!(#fname: Node),
                quote!(#fname: #bind.id()),
                quote! {
                    #[getter] fn #fname(&self, py: Python<'_>) -> PyResult<#item_ty> { Ok(#build) }
                },
                true,
            )
        }
        Shape::Child(Card::Opt, ty) => {
            let conv = classify_child(ty, reg);
            let item_ty = child_item_ty(&conv);
            let build = child_build(&conv, &quote!(&self.model), quote!(c));
            (
                quote!(#fname: Option<Node>),
                quote!(#fname: #bind.as_ref().map(|v| v.id())),
                quote! {
                    #[getter] fn #fname(&self, py: Python<'_>) -> PyResult<Option<#item_ty>> {
                        match self.#fname { Some(c) => Ok(Some(#build)), None => Ok(None) }
                    }
                },
                true,
            )
        }
        Shape::Child(Card::Vec, ty) => {
            let conv = classify_child(ty, reg);
            let item_ty = child_item_ty(&conv);
            let build = child_build(&conv, &quote!(&self.model), quote!(*c));
            (
                quote!(#fname: Vec<Node>),
                quote!(#fname: #bind.iter().map(|v| v.id()).collect()),
                quote! {
                    #[getter] fn #fname(&self, py: Python<'_>) -> PyResult<Vec<#item_ty>> {
                        self.#fname.iter().map(|c| Ok(#build)).collect()
                    }
                },
                true,
            )
        }
        Shape::Child(Card::OptVec, ty) => {
            let conv = classify_child(ty, reg);
            let item_ty = child_item_ty(&conv);
            let build = child_build(&conv, &quote!(&self.model), quote!(*c));
            (
                quote!(#fname: Option<Vec<Node>>),
                quote!(#fname: #bind.as_ref().map(|vs| vs.iter().map(|v| v.id()).collect())),
                quote! {
                    #[getter] fn #fname(&self, py: Python<'_>) -> PyResult<Option<Vec<#item_ty>>> {
                        match &self.#fname {
                            Some(cs) => Ok(Some(cs.iter().map(|c| Ok(#build)).collect::<PyResult<Vec<_>>>()?)),
                            None => Ok(None),
                        }
                    }
                },
                true,
            )
        }
        Shape::Kind(_) | Shape::Skip => (quote!(#fname: ()), quote!(#fname: ()), quote!(), false),
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn expand(input: TokenStream) -> TokenStream {
    let dsl = match syn::parse2::<Dsl>(input) {
        Ok(d) => d,
        Err(e) => return e.to_compile_error(),
    };
    let reg = build_registry(dsl);
    let walk = emit_walk(&reg);
    let py = emit_py(&reg);
    quote! {
        use crate::ir_core::Loc;
        use crate::model::Model;
        use crate::visitor::NodeVisitor;
        use fpp_ast::AstNode as _;
        use fpp_core::Node;
        use pyo3::prelude::*;
        use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyclass_enum, gen_stub_pymethods};
        use crate::lower_core::Walker;

        #walk
        #py
    }
}
