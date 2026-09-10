//! Structured diagnostics: a collecting emitter that captures each
//! `fpp_core::DiagnosticData` as owned, context-free data, plus the `Diagnostic`
//! pyclass exposed on `Model`.
//!
//! Locations are resolved during `emit` (inside the `run` scope) by upgrading
//! the span's weak file reference — no thread-local lookup is needed afterward.

use crate::ir_core::Loc;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};

/// An owned, context-free diagnostic.
#[derive(Clone)]
pub struct OwnedDiagnostic {
    pub level: &'static str,
    pub message: String,
    pub location: Option<Loc>,
    pub children: Vec<(String, Option<Loc>)>,
}

impl OwnedDiagnostic {
    pub fn is_error(&self) -> bool {
        self.level == "error"
    }
}

/// A cloneable `fpp_core::DiagnosticEmitter` that collects diagnostics as owned,
/// context-free data. It is owned by the `CompilerContext` (which we keep alive
/// in `ModelData` for lazy reflection); `run_pipeline` retains a clone to drain
/// the collected diagnostics after analysis. `Arc<Mutex<_>>` keeps it
/// `Send + Sync` so the context can live in the `Sync` `Model` pyclass.
#[derive(Clone, Default)]
pub struct SharedEmitter {
    diags: std::sync::Arc<std::sync::Mutex<Vec<OwnedDiagnostic>>>,
}

impl SharedEmitter {
    /// Drain the collected diagnostics.
    pub fn take(&self) -> Vec<OwnedDiagnostic> {
        std::mem::take(&mut self.diags.lock().unwrap())
    }
}

fn level_str(level: fpp_core::Level) -> &'static str {
    match level {
        fpp_core::Level::Error => "error",
        fpp_core::Level::Warning => "warning",
        fpp_core::Level::Note => "note",
        fpp_core::Level::Help => "help",
        _ => "note", // Level is #[non_exhaustive]
    }
}

fn loc_of(span: &fpp_core::SpanData) -> Option<Loc> {
    let file = span.file.upgrade()?;
    let start = file.position(span.start);
    let end = file.position(span.start + span.length);
    Some(Loc {
        uri: file.uri.clone(),
        line: start.line(),
        column: start.column(),
        end_line: end.line(),
        end_column: end.column(),
    })
}

impl fpp_core::DiagnosticEmitter for SharedEmitter {
    fn emit(&mut self, diagnostic: fpp_core::DiagnosticData) {
        let children = diagnostic
            .children
            .iter()
            .map(|c| (c.message.clone(), c.span.as_ref().and_then(loc_of)))
            .collect();
        self.diags.lock().unwrap().push(OwnedDiagnostic {
            level: level_str(diagnostic.level),
            message: diagnostic.message,
            location: loc_of(&diagnostic.span),
            children,
        });
    }
}

/// A diagnostic surfaced to Python.
#[gen_stub_pyclass]
#[pyclass(frozen, get_all)]
pub struct Diagnostic {
    pub level: String,
    pub message: String,
    pub location: Option<Loc>,
}

#[gen_stub_pymethods]
#[pymethods]
impl Diagnostic {
    fn __repr__(&self) -> String {
        match &self.location {
            Some(l) => format!(
                "<Diagnostic {} {}:{}:{}: {}>",
                self.level,
                l.uri,
                l.line + 1,
                l.column + 1,
                self.message
            ),
            None => format!("<Diagnostic {}: {}>", self.level, self.message),
        }
    }
}

impl Diagnostic {
    pub fn from_owned(d: &OwnedDiagnostic) -> Diagnostic {
        Diagnostic {
            level: d.level.to_string(),
            message: d.message.clone(),
            location: d.location.clone(),
        }
    }
}
