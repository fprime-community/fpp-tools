//! The two front doors — `parse` and `analyze` — and the input coercion they
//! share. They differ only in [`Depth`].
//!
//! The `CompilerContext` each call creates is **not** dropped when `run` returns:
//! it is moved into the resulting [`ModelData`] so locations and annotations
//! resolve lazily via `run_ref` at getter time.
//!
//! Include resolution runs at *both* depths: it reads files, but it is a purely
//! syntactic splice, and without it a syntax tree would have `SpecInclude` holes
//! where members belong.

use crate::diagnostics::{OwnedDiagnostic, SharedEmitter};
use crate::ir_core::{ModelData, UnitData};
use crate::lower_core;
use crate::model::{Model, SyntaxTree};
use fpp_core::FileReader;
use pyo3::exceptions::{PyOSError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyString;
use pyo3_stub_gen::derive::gen_stub_pyfunction;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// One or more `.fpp` file paths: `str` for a single file, any iterable of `str`
/// for several.
///
/// The hand-rolled `FromPyObject` + [`pyo3_stub_gen::PyStubType`] pair (like the
/// union `*Ref` newtypes in `crate::sem`, but for an *argument*) is what makes the
/// stub render `str | list[str]` instead of the `Any` a bare `Bound<PyAny>` gives.
pub struct Paths(Vec<String>);

impl<'py> FromPyObject<'_, 'py> for Paths {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        // Before the iterable protocol: a `str` is itself iterable, over its chars.
        if let Ok(s) = obj.cast::<PyString>() {
            return Ok(Paths(vec![s.extract()?]));
        }
        let mut paths = Vec::new();
        for item in obj.try_iter().map_err(|_| {
            PyValueError::new_err("expected a file path (str) or an iterable of file paths")
        })? {
            paths.push(item?.extract::<String>()?);
        }
        Ok(Paths(paths))
    }
}

impl pyo3_stub_gen::PyStubType for Paths {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo::builtin("str") | pyo3_stub_gen::TypeInfo::list_of::<String>()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Depth {
    /// Parse + include resolution.
    Syntax,
    /// …then `add_state_enums` + `check_semantics`.
    Full,
}

/// One `(uri, content)` pair per translation unit.
///
/// Reads happen here — outside the `run` scope — so an unreadable path raises
/// `OSError` before any compiler work starts.
fn collect_sources(
    paths: Option<Paths>,
    source: Option<String>,
    uri: &str,
) -> PyResult<Vec<(String, String)>> {
    let mut sources = Vec::new();
    if let Some(Paths(paths)) = paths {
        let reader = fpp_fs::FsReader {};
        for path in paths {
            let content = reader
                .read(&path)
                .map_err(|e| PyOSError::new_err(e.to_string()))?;
            sources.push((path, content));
        }
    }
    if let Some(source) = source {
        sources.push((uri.to_owned(), source));
    }
    if sources.is_empty() {
        return Err(PyValueError::new_err(
            "nothing to compile: pass at least one file path or source=...",
        ));
    }
    Ok(sources)
}

/// The recording walk runs last, after every AST mutation, so the pointers it
/// records stay valid for the life of the returned [`ModelData`] — this ordering
/// is what [`crate::noderef`]'s soundness argument rests on.
fn run_pipeline(sources: Vec<(String, String)>, depth: Depth) -> (ModelData, Vec<OwnedDiagnostic>) {
    let emitter = SharedEmitter::default();
    let mut ctx = fpp_core::CompilerContext::new(emitter.clone());
    let (units, analysis, tables, by_qualified_name) = fpp_core::run(&mut ctx, || {
        // `resolve_includes` needs an `Analysis` only for its `include_context_map`,
        // so the same one accumulates across all units.
        let mut analysis = fpp_analysis::Analysis::new();
        let mut units: Vec<UnitData> = sources
            .into_iter()
            .map(|(uri, content)| {
                let src = fpp_core::SourceFile::new(&uri, content);
                let mut tu = fpp_parser::parse(src, |p| p.trans_unit(), None);
                let _ = fpp_analysis::resolve_includes(&mut analysis, fpp_fs::FsReader {}, &mut tu);
                if depth == Depth::Full {
                    fpp_analysis::add_state_enums(&mut tu);
                }
                UnitData {
                    uri,
                    tu,
                    roots: Vec::new(),
                }
            })
            .collect();

        // All units at once, so a definition in one resolves uses in any other.
        if depth == Depth::Full {
            let _ =
                fpp_analysis::check_semantics(&mut analysis, units.iter().map(|u| &u.tu).collect());
        }

        // Last, now that no AST will change again. Collecting the roots first keeps
        // the `&u.tu` borrows out of the way of the `roots` writes; neither step
        // moves a `UnitData`.
        let mut walker = lower_core::Walker::new();
        let roots: Vec<Vec<fpp_core::Node>> = units
            .iter()
            .map(|u| crate::ast::walk_trans_unit(&mut walker, &u.tu))
            .collect();
        for (unit, roots) in units.iter_mut().zip(roots) {
            unit.roots = roots;
        }

        let tables = walker.finish();
        let by_qualified_name = lower_core::build_indexes(&analysis, &tables.ids);
        (units, analysis, tables, by_qualified_name)
    });
    let data = ModelData {
        units,
        analysis,
        ctx: std::sync::Arc::new(ctx),
        ids: tables.ids,
        node_ptrs: tables.node_ptrs,
        children: tables.children,
        by_qualified_name,
    };
    (data, emitter.take())
}

/// `catch_unwind` because letting a compiler panic unwind into the interpreter
/// aborts the process.
fn run_off_gil(
    py: Python<'_>,
    sources: Vec<(String, String)>,
    depth: Depth,
) -> PyResult<(ModelData, Vec<OwnedDiagnostic>)> {
    py.detach(move || catch_unwind(AssertUnwindSafe(move || run_pipeline(sources, depth))))
        .map_err(|_| PyRuntimeError::new_err("internal FPP compiler panic"))
}

/// Parse FPP sources into a syntax tree, without semantic analysis.
///
/// `paths` is a single `.fpp` file path or an iterable of them; `source` is
/// in-memory FPP text labelled `uri`. Either or both may be given, and each input
/// becomes one translation unit. `include` directives are resolved, which reads
/// the included files.
///
/// This is the fast front end: the nodes it yields carry locations, annotations,
/// and children, but no resolved symbols, types, or values. Use `analyze` for
/// those.
///
/// Raises `OSError` if a path cannot be read, `ValueError` if no input is given.
#[gen_stub_pyfunction]
#[pyfunction]
#[pyo3(signature = (paths = None, *, source = None, uri = "<string>"))]
fn parse(
    py: Python<'_>,
    paths: Option<Paths>,
    source: Option<String>,
    uri: &str,
) -> PyResult<Py<SyntaxTree>> {
    let sources = collect_sources(paths, source, uri)?;
    let (data, diags) = run_off_gil(py, sources, Depth::Syntax)?;
    SyntaxTree::build(py, data, diags)
}

/// Parse and semantically analyze FPP sources, returning a `Model`.
///
/// Takes the same inputs as `parse`, one translation unit per input, but analyzes
/// all units **together**, so a definition in one resolves uses in another.
///
/// `model.ast` is the *transformed* AST — the parsed units after include
/// resolution and the state-enum transform — and `model.analysis` is the analysis
/// computed from it.
///
/// Raises `OSError` if a path cannot be read, `ValueError` if no input is given.
#[gen_stub_pyfunction]
#[pyfunction]
#[pyo3(signature = (paths = None, *, source = None, uri = "<string>"))]
fn analyze(
    py: Python<'_>,
    paths: Option<Paths>,
    source: Option<String>,
    uri: &str,
) -> PyResult<Py<Model>> {
    let sources = collect_sources(paths, source, uri)?;
    let (data, diags) = run_off_gil(py, sources, Depth::Full)?;
    Py::new(py, Model::new(data, diags))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(analyze, m)?)?;
    Ok(())
}
