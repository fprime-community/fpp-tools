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
//! methods of [`crate::visitor`]'s `NodeVisitor`, whose traversal logic is
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

use diagnostics::Diagnostic;
use model::{Model, SyntaxTree, TransUnit};

#[pymodule]
fn fpp(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pipeline::register(m)?;
    m.add_class::<Model>()?;
    m.add_class::<SyntaxTree>()?;
    m.add_class::<TransUnit>()?;
    m.add_class::<Diagnostic>()?;
    m.add_class::<ir_core::Loc>()?;
    m.add_class::<ir_core::Span>()?;
    m.add_class::<visitor::NodeVisitor>()?;
    ast::register(m)?;
    sem::register(m)?;
    Ok(())
}

/// Gather the pyo3-stub-gen [`StubInfo`] for this extension.
pub fn stub_info() -> pyo3_stub_gen::Result<pyo3_stub_gen::StubInfo> {
    let manifest_dir: &std::path::Path = env!("CARGO_MANIFEST_DIR").as_ref();
    pyo3_stub_gen::StubInfo::from_pyproject_toml(manifest_dir.join("pyproject.toml"))
}

/// `(alias name, `Sub1 | Sub2 | …` RHS)` for every closed-union type. Consumed by
/// the `stub_gen` binary to inject `<Alias>: typing.TypeAlias = …` lines that
/// pyo3-stub-gen cannot express natively.
pub fn union_aliases() -> Vec<(&'static str, String)> {
    sem::union_aliases()
}
