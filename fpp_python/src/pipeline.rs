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
    /// ...then `add_state_enums` + `check_semantics`.
    Full,
}

/// One translation unit's input.
struct UnitInput {
    uri: String,
    content: String,
    /// `false` for a unit supplied only to resolve references (`imports=`).
    is_source: bool,
}

/// One [`UnitInput`] per translation unit: the sources first (paths, then `source=`),
/// then the imports.
///
/// Reads happen here — outside the `run` scope — so an unreadable path raises
/// `OSError` before any compiler work starts.
fn collect_sources(
    paths: Option<Paths>,
    source: Option<String>,
    uri: &str,
    imports: Option<Paths>,
) -> PyResult<Vec<UnitInput>> {
    let read = |path: String, is_source: bool| -> PyResult<UnitInput> {
        let content = fpp_fs::FsReader {}
            .read(&path)
            .map_err(|e| PyOSError::new_err(e.to_string()))?;
        Ok(UnitInput {
            uri: path,
            content,
            is_source,
        })
    };
    let mut sources = Vec::new();
    if let Some(Paths(paths)) = paths {
        for path in paths {
            sources.push(read(path, true)?);
        }
    }
    if let Some(source) = source {
        sources.push(UnitInput {
            uri: uri.to_owned(),
            content: source,
            is_source: true,
        });
    }
    // Sources only: `imports=` alone is nothing to compile, since an import is
    // present to be referenced, not to be processed.
    if sources.is_empty() {
        return Err(PyValueError::new_err(
            "nothing to compile: pass at least one file path or source=...",
        ));
    }
    if let Some(Paths(imports)) = imports {
        for path in imports {
            sources.push(read(path, false)?);
        }
    }
    Ok(sources)
}

/// The recording walk runs last, after every AST mutation, so the pointers it
/// records stay valid for the life of the returned [`ModelData`] — this ordering
/// is what [`crate::noderef`]'s soundness argument rests on.
fn run_pipeline(inputs: Vec<UnitInput>, depth: Depth) -> (ModelData, Vec<OwnedDiagnostic>) {
    let emitter = SharedEmitter::default();
    let mut ctx = fpp_core::CompilerContext::new(emitter.clone());
    let (units, analysis, tables, by_qualified_name) = fpp_core::run(&mut ctx, || {
        // `resolve_includes` needs an `Analysis` only for its `include_context_map`,
        // so the same one accumulates across all units.
        let mut analysis = fpp_analysis::Analysis::new();
        let mut units: Vec<UnitData> = inputs
            .into_iter()
            .map(|input| {
                let src = fpp_core::SourceFile::new(&input.uri, input.content);
                let mut tu = fpp_parser::parse(src, |p| p.trans_unit(), None);
                let _ = fpp_analysis::resolve_includes(&mut analysis, fpp_fs::FsReader {}, &mut tu);
                if depth == Depth::Full {
                    fpp_analysis::add_state_enums(&mut tu);
                }
                UnitData {
                    uri: input.uri,
                    tu,
                    roots: Vec::new(),
                    is_source: input.is_source,
                    // Filled by the walk below.
                    id_range: 0..0,
                }
            })
            .collect();

        // All units at once, so a definition in one resolves uses in any other —
        // which is the whole point of an import: it is analyzed exactly like a
        // source, and only `UnitData::is_source` tells them apart afterwards.
        if depth == Depth::Full {
            let _ =
                fpp_analysis::check_semantics(&mut analysis, units.iter().map(|u| &u.tu).collect());
        }

        // Last, now that no AST will change again. Collecting the roots first keeps
        // the `&u.tu` borrows out of the way of the `roots` writes; neither step
        // moves a `UnitData`.
        let mut walker = lower_core::Walker::new();
        let walked: Vec<(Vec<fpp_core::Node>, std::ops::Range<u32>)> = units
            .iter()
            .map(|u| {
                let start = walker.next_id();
                let roots = crate::ast::walk_trans_unit(&mut walker, &u.tu);
                (roots, start..walker.next_id())
            })
            .collect();
        for (unit, (roots, id_range)) in units.iter_mut().zip(walked) {
            unit.roots = roots;
            unit.id_range = id_range;
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
    inputs: Vec<UnitInput>,
    depth: Depth,
) -> PyResult<(ModelData, Vec<OwnedDiagnostic>)> {
    py.detach(move || catch_unwind(AssertUnwindSafe(move || run_pipeline(inputs, depth))))
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
/// There is no `imports=` here, unlike `analyze`: nothing at this depth reads
/// another translation unit, so every unit `parse` returns is a source.
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
    let sources = collect_sources(paths, source, uri, None)?;
    let (data, diags) = run_off_gil(py, sources, Depth::Syntax)?;
    SyntaxTree::build(py, data, diags)
}

/// Parse and semantically analyze FPP sources, returning a `Model`.
///
/// Takes the same inputs as `parse`, one translation unit per input, but analyzes
/// all units **together**, so a definition in one resolves uses in another.
///
/// `imports` names units that are present only to resolve references — the
/// counterpart of `fpp-to-cpp -i`. Their units answer `is_source == False`, as does
/// `AstNode.in_source` for the nodes inside them. So the units a caller asked
/// about are `[u for u in model.ast if u.is_source]`.
/// `imports` alone is not enough to compile: it raises `ValueError`.
///
/// `model.ast` is the *transformed* AST after running analysis transformations
///
/// Raises `OSError` if a path cannot be read, `ValueError` if no source is given.
#[gen_stub_pyfunction]
#[pyfunction]
#[pyo3(signature = (paths = None, *, source = None, uri = "<string>", imports = None))]
fn analyze(
    py: Python<'_>,
    paths: Option<Paths>,
    source: Option<String>,
    uri: &str,
    imports: Option<Paths>,
) -> PyResult<Py<Model>> {
    let sources = collect_sources(paths, source, uri, imports)?;
    let (data, diags) = run_off_gil(py, sources, Depth::Full)?;
    Py::new(py, Model::new(data, diags))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(analyze, m)?)?;
    Ok(())
}
