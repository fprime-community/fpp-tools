//! Function-like macros that expand the declarative `fpp_ast` / `fpp_analysis`
//! grammar mirrors (checked in as `defs.rs` declaration files) into the PyO3
//! wrappers for the `fpp_python` crate. These macros only emit scaffolding
//! (struct shape, constructors, uniform getters, dispatch, walk) — they never
//! mirror the source data, so the generated wrappers still read the live
//! `fpp_ast` nodes / `fpp_analysis` `Analysis` directly.
//!
//! * [`macro@fpp_ast_bindings`] — the AST-node wrappers + recording walk.
//! * [`macro@fpp_sem_bindings`] — the semantic-layer wrappers (the
//!   `Symbol`/`Type`/`Value`/`StateMachineElement` closed unions, the nested/thin
//!   entity layer, and the top-level symbol-keyed entities).

use proc_macro::TokenStream;

mod ast_bindings;
mod sem_bindings;

/// Expand a declarative mirror of the `fpp_ast` grammar (emitted by the
/// `bindgen` binary into `fpp_python/src/ast/defs.rs`) into the PyO3 AST-node
/// wrappers + the recording walk. The DSL has `leaves {…}`, `shadowed {…}`,
/// `node X { field: Type, … }`, `union X { Variant(Inner), … }`, and
/// `kind X { … }` sections; cardinality is `T` / `T?` / `[T]` / `[T]?`. See
/// [`mod@ast_bindings`] for the grammar + classification rules.
#[proc_macro]
pub fn fpp_ast_bindings(input: TokenStream) -> TokenStream {
    ast_bindings::expand(input.into()).into()
}

/// Expand a declarative mirror of the `fpp_analysis` semantic layer (emitted by
/// `bindgen` into `fpp_python/src/sem/defs.rs`) into the read-only PyO3 wrappers for the
/// semantic data structures: the `Symbol`/`Type`/`Value` closed-union
/// hierarchies, the entity structs (nested/thin + top-level symbol-keyed), their
/// union `*Ref` newtypes + Python aliases, and the leaf-enum mirrors. See
/// [`mod@sem_bindings`] for the grammar + classification rules.
#[proc_macro]
pub fn fpp_sem_bindings(input: TokenStream) -> TokenStream {
    sem_bindings::expand(input.into()).into()
}
