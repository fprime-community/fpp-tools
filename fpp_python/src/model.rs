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
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyType;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use rustc_hash::FxHashMap;
use std::sync::{Arc, Mutex};

/// The `kind=` argument of [`Model::lookup`]/[`Model::lookup_all`]: a symbol class
/// to filter on.
pub struct SymbolKind(Py<PyType>);

impl<'py> FromPyObject<'_, 'py> for SymbolKind {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let ty = obj.cast::<PyType>().map_err(|_| {
            PyTypeError::new_err(format!(
                "kind must be a symbol class, not {}",
                obj.get_type()
                    .name()
                    .map_or_else(|_| "an unnameable object".to_string(), |n| n.to_string())
            ))
        })?;
        if !ty.is_subclass_of::<crate::sem::Symbol>()? {
            return Err(PyTypeError::new_err(format!(
                "kind must be a symbol class (a subclass of SymbolBase), not `{}`",
                ty.name()?
            )));
        }
        Ok(SymbolKind(ty.to_owned().unbind()))
    }
}

impl pyo3_stub_gen::PyStubType for SymbolKind {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        // `Symbol` is the injected union alias over the fifteen symbol classes; the
        // parameter is the class itself, not an instance.
        let mut info = pyo3_stub_gen::TypeInfo::unqualified("Symbol");
        info.name = format!("type[{}]", info.name);
        info
    }
}

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
        let error_count = diagnostics
            .iter()
            .filter(|d| crate::diagnostics::is_error(d))
            .count();
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

    /// The symbols named `qualified_name` that are instances of `kind`, in
    /// declaration order — at most one when `first_only`.
    ///
    /// The filter is an `isinstance` check on the *built* wrapper, so it needs no
    /// knowledge of the fifteen symbol classes and stays correct if any of them is
    /// renamed. Building a candidate is cheap (the wrapper stores a cloned handle),
    /// and `first_only` stops after the first match, so the common single-symbol
    /// lookup builds exactly one.
    fn matching(
        slf: PyRef<'_, Self>,
        qualified_name: &str,
        kind: Option<&SymbolKind>,
        first_only: bool,
    ) -> PyResult<Vec<crate::sem::SymbolRef>> {
        let py = slf.py();
        let syms = match slf.data.by_qualified_name.get(qualified_name) {
            Some(syms) => syms.clone(),
            None => return Ok(Vec::new()),
        };
        let model: Py<Self> = slf.into();
        let mut out = Vec::new();
        for sym in syms {
            let built = crate::sem::build_symbol(&model, py, sym)?
                .into_bound(py)
                .into_any();
            if let Some(kind) = kind
                && !built.is_instance(kind.0.bind(py))?
            {
                continue;
            }
            out.push(crate::sem::SymbolRef(built.unbind()));
            if first_only {
                break;
            }
        }
        Ok(out)
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
    ///
    /// Freshly built on each read, so writing to one of them (a `Diagnostic` is
    /// mutable) changes nothing in the model.
    #[getter]
    fn diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics
            .iter()
            .map(Diagnostic::from_owned)
            .collect()
    }

    /// Look up a symbol by its fully-qualified (dotted) name.
    ///
    /// One name can denote more than one symbol: FPP keeps types and ports in
    /// separate name groups, so `Fw.Time` in F Prime's own `Fw/Time/Time.fpp` is
    /// both a `type Time` and a `port Time`. This returns the **first declared** of
    /// them; pass `kind` to say which you want, or use `lookup_all` to see them
    /// all.
    ///
    /// `kind` is a symbol class — `model.lookup("Fw.Time", kind=fpp.SymbolAbsType)`
    /// — and matches subclasses, as `isinstance` does. `None` if nothing matches.
    #[pyo3(signature = (qualified_name, *, kind = None))]
    fn lookup(
        slf: PyRef<'_, Self>,
        qualified_name: &str,
        kind: Option<SymbolKind>,
    ) -> PyResult<Option<crate::sem::SymbolRef>> {
        Ok(Self::matching(slf, qualified_name, kind.as_ref(), true)?.pop())
    }

    /// Every symbol with this fully-qualified (dotted) name, in declaration order.
    ///
    /// Usually one; two when a name is declared in two name groups (see `lookup`).
    /// Empty if nothing matches.
    #[pyo3(signature = (qualified_name, *, kind = None))]
    fn lookup_all(
        slf: PyRef<'_, Self>,
        qualified_name: &str,
        kind: Option<SymbolKind>,
    ) -> PyResult<Vec<crate::sem::SymbolRef>> {
        Self::matching(slf, qualified_name, kind.as_ref(), false)
    }

    /// The semantic analysis result — the strict 1:1 mirror of
    /// `fpp_analysis::Analysis`. Navigate the model's semantics through its
    /// public maps (e.g. `model.analysis.component_map`) and methods (e.g.
    /// `model.analysis.get_qualified_name(sym)`).
    ///
    /// # Map iteration order
    ///
    /// Every `dict` the analysis hands back iterates in a defined order, so output
    /// generated by walking one is stable without sorting:
    ///
    /// * keyed by an id, a name, or a kind (`command_map`, `event_map`,
    ///   `tlm_channel_map`, `param_map`, `container_map`, `record_map`, `port_map`,
    ///   `pattern_map`, …) → **key order**;
    /// * keyed by anything that denotes an AST node — a node id, a symbol, a
    ///   state-machine typed element (`type_map`, `value_map`, `use_def_map`,
    ///   `component_map`, `topology_map`, `symbol_map`, `type_option_map`, …) →
    ///   **definition order**, i.e. the order the definitions appear in the source;
    /// * keyed by a connection or a port-instance identifier (`Topology`'s
    ///   `instance_map`, `output_connection_map`, `input_connection_map`,
    ///   `from_port_number_map`, `to_port_number_map`) → **key order**, from the
    ///   ordered map the analysis already keeps them in.
    ///
    /// One exception: `TransitionGraph.arc_map`, whose key neither sorts nor carries
    /// a node id, comes out in the underlying hash order — stable from run to run,
    /// but neither key nor declaration order. Sort it yourself if you emit from it.
    ///
    /// One more caveat: a *semantic* map keyed by member name — chiefly
    /// `AnonStructType.members` — is in name order, not declaration order, because
    /// the analysis stores those members unordered and so has no declaration order
    /// to give. Read `struct_type.node.members` for the declared order.
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

    /// Whether this is a unit the caller asked about, rather than one passed to
    /// `analyze(imports=…)` only to resolve references.
    ///
    /// Always `True` for a unit from `parse`, which has no imports.
    #[getter]
    fn is_source(&self) -> bool {
        self.data.units[self.index].is_source
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
