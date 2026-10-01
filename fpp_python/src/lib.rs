//! Native PyO3 bindings to the Rust FPP compiler (`fpp-tools`).
//!
//! Two entry points, both in [`pipeline`] and both taking `.fpp` paths and/or
//! in-memory source, one translation unit each: `parse()` stops after include
//! resolution and returns a [`model::SyntaxTree`]; `analyze()` runs the full
//! pipeline and returns a [`model::Model`] carrying the transformed AST and the
//! analysis.
//!
//! Each call runs in one `fpp_core::run` scope and yields an owned
//! [`ir_core::ModelData`]: the *live* parsed ASTs, `fpp_analysis::Analysis`, and
//! `fpp_core::CompilerContext`, plus small `fpp_core::Node`-keyed side-tables.
//! There is no owned per-node IR copy and no owned semantic mirror — wrappers
//! read the live nodes and the live `Analysis` directly, resolving
//! locations/annotations lazily by re-entering the retained context via
//! `fpp_core::run_ref`.
//!
//! Two wrapper layers are expanded at compile time from checked-in declaration
//! files: the AST-node wrappers + recording walk from `fpp_ast_bindings!` over
//! [`crate::ast`], and the semantic wrappers from `fpp_sem_bindings!` over
//! [`crate::sem`] (with `sem::hand` supplying `build_type`, the one escape hatch
//! the macro cannot produce). The same AST macro also emits the typed `visit_*`
//! methods of [`crate::visitor`]'s `AstVisitor`, whose traversal logic is
//! hand-written. Everything else is the hand-written core: `ir_core`,
//! `lower_core`, `noderef`, `pipeline`, `model`, `visitor`, and `diagnostics`.

use pyo3::prelude::*;

mod ast;
mod diagnostics;
mod ir_core;
mod lower_core;
mod model;
mod noderef;
mod pipeline;
mod sem;
mod visitor;

use model::{Model, SyntaxTree, TransUnit};

pub use ast::PY_MODULE as AST_MODULE;
pub use sem::PY_MODULE as PACKAGE;

#[pymodule]
fn fpp(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pipeline::register(m)?;
    m.add_class::<Model>()?;
    m.add_class::<SyntaxTree>()?;
    m.add_class::<TransUnit>()?;
    diagnostics::register(m)?;
    m.add_class::<ir_core::Loc>()?;
    m.add_class::<ir_core::Span>()?;
    m.add_class::<visitor::AstVisitor>()?;
    ast::register(m)?;
    sem::register(m)?;
    Ok(())
}

/// Gather the pyo3-stub-gen [`StubInfo`] for this extension.
///
/// Mixed layout (`python-source` in `pyproject.toml`), because the stub spans two
/// modules and pyo3-stub-gen refuses a submodule in the pure-Rust layout — it has
/// only one file to write there. The `stub_gen` binary writes the files itself, so
/// what matters here is that both modules come back resolved against [`PACKAGE`].
pub fn stub_info() -> pyo3_stub_gen::Result<pyo3_stub_gen::StubInfo> {
    let manifest_dir: &std::path::Path = env!("CARGO_MANIFEST_DIR").as_ref();
    pyo3_stub_gen::StubInfo::from_project_root(
        PACKAGE.to_string(),
        manifest_dir.join("python"),
        true,
        Default::default(),
    )
}

/// What the stub generator needs to nest one closed union.
pub struct UnionStub {
    /// The module the base class lives in, or `None` for the package's default module
    /// (every semantic union — only `crate::ast` declares a module of its own).
    pub module: Option<&'static str>,
    /// The base class's Python name.
    pub base: &'static str,
    /// The variants' `("<Base>.<Variant>", "<Variant>")` spellings, in declaration
    /// order.
    pub variants: Vec<(&'static str, &'static str)>,
    /// The `<Base>.A | …` RHS of the nested `Variant` alias.
    pub variant_rhs: String,
}

/// One [`UnionStub`] per closed union — the semantic unions and the AST kind enums,
/// which nest identically. Consumed by the `stub_gen` binary, which nests each variant
/// under its base and adds the `Variant` alias; pyo3-stub-gen derives neither.
pub fn union_stubs() -> Vec<UnionStub> {
    let mut stubs = sem::union_stubs();
    stubs.extend(ast::kind_stubs());
    stubs
}
