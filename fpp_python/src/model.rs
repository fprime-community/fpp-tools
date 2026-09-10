//! The result pyclasses and the graph root.
//!
//! [`Model`] is the root: it owns the [`ModelData`] backing, the node memo (for
//! `is`-identity), and the `build` entry point that hands out lazy node wrappers.
//! Every AST and semantic wrapper holds a `Py<Model>`, which is what keeps the
//! backing alive.
//!
//! `Model` is also what `analyze` returns. `parse` returns a [`SyntaxTree`], which
//! wraps a *private* root — the same machinery over a backing that carries no
//! semantics.
//!
//! The per-variant AST wrapper construction lives in `crate::ast::construct`.

use crate::ast::{self as py_ast, AstNode};
use crate::diagnostics::{Diagnostic, OwnedDiagnostic};
use crate::ir_core::ModelData;
use fpp_core::Node;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use rustc_hash::FxHashMap;
use std::sync::{Arc, Mutex};

/// A parsed and semantically analyzed set of translation units.
///
/// Returned by `analyze`.
#[gen_stub_pyclass]
#[pyclass]
pub struct Model {
    pub data: Arc<ModelData>,
    // Node memo giving `is`-identity across navigation and making cycles safe.
    // `Mutex` (not `RefCell`) because pyclasses must be `Sync`.
    memo: Mutex<FxHashMap<Node, Py<AstNode>>>,
    // Memoized for the same reason as `memo`: `model.ast[0] is model.ast[0]`.
    units: Mutex<Option<Vec<Py<TransUnit>>>>,
    diagnostics: Vec<OwnedDiagnostic>,
    #[pyo3(get)]
    pub has_errors: bool,
    #[pyo3(get)]
    pub error_count: usize,
}

impl Model {
    pub fn new(data: ModelData, diagnostics: Vec<OwnedDiagnostic>) -> Self {
        let error_count = diagnostics.iter().filter(|d| d.is_error()).count();
        Model {
            data: Arc::new(data),
            memo: Mutex::new(FxHashMap::default()),
            units: Mutex::new(None),
            diagnostics,
            has_errors: error_count > 0,
            error_count,
        }
    }

    /// Build (or return the memoized) Python wrapper for a node.
    pub fn build(model: &Py<Model>, py: Python<'_>, node: Node) -> PyResult<Py<AstNode>> {
        {
            let m = model.borrow(py);
            if let Some(obj) = m.memo.lock().unwrap().get(&node) {
                return Ok(obj.clone_ref(py));
            }
        }
        let obj = py_ast::construct(model, py, node)?;
        model
            .borrow(py)
            .memo
            .lock()
            .unwrap()
            .insert(node, obj.clone_ref(py));
        Ok(obj)
    }

    /// Build (or return the memoized) wrapper for each translation unit, in input
    /// order.
    pub fn units(model: &Py<Model>, py: Python<'_>) -> PyResult<Vec<Py<TransUnit>>> {
        let clone_all = |us: &[Py<TransUnit>]| -> Vec<Py<TransUnit>> {
            us.iter().map(|u| u.clone_ref(py)).collect()
        };
        {
            let m = model.borrow(py);
            if let Some(units) = m.units.lock().unwrap().as_deref() {
                return Ok(clone_all(units));
            }
        }
        let data = model.borrow(py).data.clone();
        let built: Vec<Py<TransUnit>> = (0..data.units.len())
            .map(|index| {
                Py::new(
                    py,
                    TransUnit {
                        data: data.clone(),
                        model: model.clone_ref(py),
                        index,
                    },
                )
            })
            .collect::<PyResult<_>>()?;
        let m = model.borrow(py);
        let mut slot = m.units.lock().unwrap();
        // A concurrent builder may have won the race; its wrappers are the ones
        // already handed out, so keep those and drop ours.
        Ok(clone_all(slot.get_or_insert(built)))
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl Model {
    /// The transformed AST: one `TransUnit` per input, in the order given.
    ///
    /// "Transformed" is the parsed units after include resolution and the
    /// state-enum transform — what the analysis was actually computed from.
    #[getter]
    fn ast(slf: PyRef<'_, Self>) -> PyResult<Vec<Py<TransUnit>>> {
        let py = slf.py();
        let model: Py<Self> = slf.into();
        Model::units(&model, py)
    }

    /// The diagnostics (errors, warnings, notes) emitted during analysis.
    #[getter]
    fn diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics
            .iter()
            .map(Diagnostic::from_owned)
            .collect()
    }

    /// Look up a symbol by its fully-qualified (dotted) name.
    fn lookup(
        slf: PyRef<'_, Self>,
        qualified_name: &str,
    ) -> PyResult<Option<crate::sem::SymbolRef>> {
        let py = slf.py();
        let sym = slf.data.by_qualified_name.get(qualified_name).cloned();
        let model: Py<Self> = slf.into();
        match sym {
            Some(s) => Ok(Some(crate::sem::symbol_ref(&model, py, s)?)),
            None => Ok(None),
        }
    }

    /// The semantic analysis result — the strict 1:1 mirror of
    /// `fpp_analysis::Analysis`. Navigate the model's semantics through its
    /// public maps (e.g. `model.analysis.component_map`) and methods (e.g.
    /// `model.analysis.get_qualified_name(sym)`).
    #[getter]
    fn analysis(slf: PyRef<'_, Self>) -> PyResult<Py<crate::sem::Analysis>> {
        let py = slf.py();
        let model: Py<Self> = slf.into();
        crate::sem::build_analysis(&model, py)
    }

    fn __repr__(&self) -> String {
        format!(
            "<Model units={} nodes={} errors={}>",
            self.data.units.len(),
            self.data.ids.len(),
            self.error_count
        )
    }
}

/// A parsed set of translation units, without semantic analysis.
///
/// Returned by `parse`. There is no `analysis` or `lookup`, and the nodes it
/// yields have no resolved symbols, types, or values (those getters return
/// `None`). Use `analyze` for semantics.
#[gen_stub_pyclass]
#[pyclass(frozen)]
pub struct SyntaxTree {
    // Private: `SyntaxTree` forwards only the accessors a syntax-only backing can
    // honestly answer, so a semantics-free `Model` never reaches Python.
    root: Py<Model>,
}

impl SyntaxTree {
    pub fn build(
        py: Python<'_>,
        data: ModelData,
        diagnostics: Vec<OwnedDiagnostic>,
    ) -> PyResult<Py<SyntaxTree>> {
        let root = Py::new(py, Model::new(data, diagnostics))?;
        Py::new(py, SyntaxTree { root })
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl SyntaxTree {
    /// The parsed translation units, one per input, in the order given.
    #[getter]
    fn units(&self, py: Python<'_>) -> PyResult<Vec<Py<TransUnit>>> {
        Model::units(&self.root, py)
    }

    /// The syntax diagnostics (errors, warnings, notes) emitted while parsing.
    #[getter]
    fn diagnostics(&self, py: Python<'_>) -> Vec<Diagnostic> {
        self.root.borrow(py).diagnostics()
    }

    #[getter]
    fn has_errors(&self, py: Python<'_>) -> bool {
        self.root.borrow(py).has_errors
    }

    #[getter]
    fn error_count(&self, py: Python<'_>) -> usize {
        self.root.borrow(py).error_count
    }

    fn __len__(&self, py: Python<'_>) -> usize {
        self.root.borrow(py).data.units.len()
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        let root = self.root.borrow(py);
        format!(
            "<SyntaxTree units={} nodes={} errors={}>",
            root.data.units.len(),
            root.data.ids.len(),
            root.error_count
        )
    }
}

/// One translation unit: everything parsed from a single input.
///
/// Yielded by `SyntaxTree.units` and `Model.ast`.
#[gen_stub_pyclass]
#[pyclass(frozen)]
pub struct TransUnit {
    data: Arc<ModelData>,
    model: Py<Model>,
    index: usize,
}

#[gen_stub_pymethods]
#[pymethods]
impl TransUnit {
    /// The URI this unit was parsed from: the file path, or the `uri=` argument
    /// for in-memory source.
    ///
    /// Nodes spliced in by `include` come from other files — read a node's
    /// `location.uri` for the file it actually lives in.
    #[getter]
    fn uri(&self) -> String {
        self.data.units[self.index].uri.clone()
    }

    /// This unit's top-level member nodes, in source order.
    #[getter]
    fn members(&self, py: Python<'_>) -> PyResult<Vec<Py<AstNode>>> {
        self.data.units[self.index]
            .roots
            .iter()
            .map(|n| Model::build(&self.model, py, *n))
            .collect()
    }

    fn __len__(&self) -> usize {
        self.data.units[self.index].roots.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "<TransUnit {} members={}>",
            self.data.units[self.index].uri,
            self.data.units[self.index].roots.len()
        )
    }
}
