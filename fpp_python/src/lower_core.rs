//! Hand-written core of the recording walk: the [`Walker`] that assigns each AST
//! node a dense id and records a type-tagged pointer to it. The per-node `walk_*`
//! functions and the `walk_trans_unit` entry point are expanded by
//! `fpp_python_macros::fpp_ast_bindings!` into `crate::ast`.
//!
//! Locations and annotations are **not** recorded here — the live
//! `CompilerContext` is kept alive in [`crate::ir_core::ModelData`] and resolved
//! lazily (via `fpp_core::run_ref`) at getter time. The walk runs inside the
//! pipeline's `fpp_core::run` scope over an AST that is never mutated afterwards
//! (so the recorded pointers stay valid — see `crate::noderef`).

use crate::ast::NodeKind;
use crate::ir_core::Loc;
use crate::noderef::NodeRef;
use fpp_analysis::Analysis;
use fpp_analysis::semantics::Symbol;
use fpp_core::Node;
use rustc_hash::FxHashMap;

/// The side-tables recorded by a completed walk, moved into
/// [`crate::ir_core::ModelData`].
pub struct WalkTables {
    /// Dense id per node, assigned in walk pre-order.
    pub ids: FxHashMap<Node, u32>,
    /// Type-tag + raw pointer into the walked AST per node.
    pub node_ptrs: FxHashMap<Node, NodeRef>,
    /// Direct child nodes per node, in walk order. Childless nodes are absent.
    pub children: FxHashMap<Node, Vec<Node>>,
}

/// Accumulates the side-tables while walking the AST. Ids are assigned in
/// pre-order (first visit wins), matching source order.
#[derive(Default)]
pub struct Walker {
    next: u32,
    ids: FxHashMap<Node, u32>,
    node_ptrs: FxHashMap<Node, NodeRef>,
    children: FxHashMap<Node, Vec<Node>>,
}

impl Walker {
    pub fn new() -> Self {
        Walker::default()
    }

    /// Record a node on first sight: assign its dense id and store its
    /// type-tagged pointer. Returns `true` if the node was newly seen (caller
    /// should recurse into its children), `false` if it was already recorded
    /// (shared node — skip to avoid rebuilding).
    ///
    /// # Safety
    ///
    /// `ptr` must point at a `T` of the AST type `tag` denotes, and must remain
    /// valid and immutable for the life of the resulting `ModelData` (the caller
    /// walks the owned, never-again-mutated AST — see `crate::noderef`).
    pub fn enter(&mut self, node: Node, tag: NodeKind, ptr: *const ()) -> bool {
        if self.ids.contains_key(&node) {
            return false;
        }
        let id = self.next;
        self.next += 1;
        self.ids.insert(node, id);
        self.node_ptrs.insert(node, NodeRef { tag, ptr });
        true
    }

    /// Record `node`'s direct child nodes, in walk (source) order.
    ///
    /// Called once per node, right after its children have been walked. Ids that
    /// were not themselves recorded are dropped: a union variant whose inner
    /// type is not a walked node yields an id with no dense id or pointer, and
    /// such a child could not be built into a wrapper.
    pub fn set_children(&mut self, node: Node, mut children: Vec<Node>) {
        children.retain(|c| self.ids.contains_key(c));
        if !children.is_empty() {
            self.children.insert(node, children);
        }
    }

    /// Consume the walker, yielding the recorded side-tables.
    pub fn finish(self) -> WalkTables {
        WalkTables {
            ids: self.ids,
            node_ptrs: self.node_ptrs,
            children: self.children,
        }
    }
}

/// Build the fully-qualified-name -> [`Symbol`] index for `Model.lookup`, inside
/// the `run` scope. Only symbols whose def node was recorded during the walk are
/// included (synthetic/unwalked nodes are skipped). `get_qualified_name` is
/// context-free, but building the index once here keeps lookups O(1).
pub fn build_indexes(a: &Analysis, ids: &FxHashMap<Node, u32>) -> FxHashMap<String, Symbol> {
    let mut by_qualified_name: FxHashMap<String, Symbol> = FxHashMap::default();
    for (def_node, sym) in &a.symbol_map {
        if ids.contains_key(def_node) {
            by_qualified_name.insert(a.get_qualified_name(sym), sym.clone());
        }
    }
    by_qualified_name
}

/// Resolve a `fpp_core::Span` to an owned [`Loc`]. Must be called inside a
/// `run`/`run_ref` scope (it reads the span's file + positions through the
/// thread-local compiler context).
pub fn loc_of_span(span: &fpp_core::Span) -> Loc {
    let start = span.start();
    let end = span.end();
    Loc {
        uri: span.file().uri(),
        line: start.line(),
        column: start.column(),
        end_line: end.line(),
        end_column: end.column(),
    }
}
