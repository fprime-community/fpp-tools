//! Structured diagnostics: a collecting emitter that captures each
//! `fpp_core::DiagnosticData` as owned, context-free data, plus the
//! [`Diagnostic`]/[`DiagnosticMessage`] pyclasses exposed on `Model`.
//!
//! The owned form is `fpp_errors::OwnedDiagnostic`: every span is resolved to its
//! source excerpt during `emit` (inside the `run` scope, where the span's weak
//! file reference is still upgradable), so nothing here needs the compiler
//! context afterward. Rendering goes back through `fpp_errors`, so
//! `str(diagnostic)` is the same text the `fpp` console prints.

use crate::ir_core::{Loc, Span};
use fpp_core::{DiagnosticDataSnippet, Level};
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyclass_enum, gen_stub_pymethods};

/// An owned, context-free diagnostic — what the emitter collects and what
/// [`Diagnostic`] wraps.
pub use fpp_errors::{OwnedDiagnostic, OwnedDiagnosticMessage};

/// Whether `diagnostic` is an error (as opposed to a warning or a note).
pub fn is_error(diagnostic: &OwnedDiagnostic) -> bool {
    diagnostic.level == Level::Error
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

impl fpp_core::DiagnosticEmitter for SharedEmitter {
    fn emit(&mut self, diagnostic: fpp_core::DiagnosticData) {
        self.diags
            .lock()
            .unwrap()
            .push(OwnedDiagnostic::from(&diagnostic));
    }
}

/// A diagnostic's severity
#[gen_stub_pyclass_enum]
#[pyclass(eq, eq_int, frozen, hash, from_py_object)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Note,
    Help,
}

impl DiagnosticLevel {
    fn member_name(&self) -> &'static str {
        match self {
            DiagnosticLevel::Error => "Error",
            DiagnosticLevel::Warning => "Warning",
            DiagnosticLevel::Note => "Note",
            DiagnosticLevel::Help => "Help",
        }
    }

    fn spelling(&self) -> &'static str {
        match self {
            DiagnosticLevel::Error => "error",
            DiagnosticLevel::Warning => "warning",
            DiagnosticLevel::Note => "note",
            DiagnosticLevel::Help => "help",
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl DiagnosticLevel {
    /// Get enum variant name as a string
    #[getter]
    fn name(&self) -> &'static str {
        self.member_name()
    }

    /// The compiler's spelling of this level: `"error"`, `"warning"`, `"note"`
    /// or `"help"`.
    #[getter]
    fn value(&self) -> &'static str {
        self.spelling()
    }
}

impl From<Level> for DiagnosticLevel {
    fn from(level: Level) -> DiagnosticLevel {
        match level {
            Level::Error => DiagnosticLevel::Error,
            Level::Warning => DiagnosticLevel::Warning,
            Level::Help => DiagnosticLevel::Help,
            // `Level` is `#[non_exhaustive]`; a note is the least assuming
            // reading of a level this mirror does not know.
            _ => DiagnosticLevel::Note,
        }
    }
}

impl From<DiagnosticLevel> for Level {
    fn from(level: DiagnosticLevel) -> Level {
        match level {
            DiagnosticLevel::Error => Level::Error,
            DiagnosticLevel::Warning => Level::Warning,
            DiagnosticLevel::Note => Level::Note,
            DiagnosticLevel::Help => Level::Help,
        }
    }
}

/// What a `DiagnosticMessageKind` is: a further annotation at its diagnostic's own
/// level (the compiler's `span_annotation`/`annotation`), or a note
/// (`span_note`/`note`).
#[gen_stub_pyclass_enum]
#[pyclass(eq, eq_int, frozen, hash, from_py_object)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticMessageKind {
    Annotation,
    Note,
}

impl DiagnosticMessageKind {
    fn member_name(&self) -> &'static str {
        match self {
            DiagnosticMessageKind::Annotation => "Annotation",
            DiagnosticMessageKind::Note => "Note",
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl DiagnosticMessageKind {
    /// Get enum variant name as a string
    #[getter]
    fn name(&self) -> &'static str {
        self.member_name()
    }

    /// The same string as `name`: a kind mirror carries no payload, so its name
    /// is its value (as for `enum.StrEnum`).
    #[getter]
    fn value(&self) -> &'static str {
        self.member_name()
    }
}

impl From<fpp_core::DiagnosticMessageKind> for DiagnosticMessageKind {
    fn from(kind: fpp_core::DiagnosticMessageKind) -> DiagnosticMessageKind {
        match kind {
            fpp_core::DiagnosticMessageKind::Primary => DiagnosticMessageKind::Annotation,
            fpp_core::DiagnosticMessageKind::Note => DiagnosticMessageKind::Note,
        }
    }
}

impl From<DiagnosticMessageKind> for fpp_core::DiagnosticMessageKind {
    fn from(kind: DiagnosticMessageKind) -> fpp_core::DiagnosticMessageKind {
        match kind {
            DiagnosticMessageKind::Annotation => fpp_core::DiagnosticMessageKind::Primary,
            DiagnosticMessageKind::Note => fpp_core::DiagnosticMessageKind::Note,
        }
    }
}

/// The 0-indexed line/column in the original file of `offset`, a byte offset into
/// `snippet`'s excerpt.
///
/// The excerpt spans whole lines starting at `line_offset`, so the line is that
/// plus the newlines before `offset`, and the column is the distance back to the
/// last of them.
fn position_in(snippet: &DiagnosticDataSnippet, offset: u32) -> (u32, u32) {
    let head = &snippet.file_content[..offset as usize];
    let line = snippet.line_offset as u32 + head.matches('\n').count() as u32;
    let column = match head.rfind('\n') {
        Some(last) => offset - (last as u32 + 1),
        None => offset,
    };
    (line, column)
}

/// The span an excerpt was taken from, as a [`Loc`].
fn loc_of_snippet(snippet: &DiagnosticDataSnippet) -> Loc {
    let (line, column) = position_in(snippet, snippet.start);
    let (end_line, end_column) = position_in(snippet, snippet.end);
    Loc {
        uri: snippet.uri.clone(),
        line,
        column,
        end_line,
        end_column,
    }
}

/// The chain of `include` specifiers that spliced an excerpt's source in, nearest
/// first. Each is a point, so its end equals its start.
fn includes_of_snippet(snippet: &DiagnosticDataSnippet) -> Vec<Loc> {
    snippet
        .include_spans
        .iter()
        .map(|include| Loc {
            uri: include.uri.clone(),
            line: include.line,
            column: include.column,
            end_line: include.line,
            end_column: include.column,
        })
        .collect()
}

/// The excerpt a `Span` argument points at, or `None` for `span=None`.
fn snippet_of_span(py: Python<'_>, span: Option<Py<Span>>) -> Option<DiagnosticDataSnippet> {
    span.map(|span| span.borrow(py).snippet())
}

/// One child message of a `Diagnostic`: a second span worth pointing at, or a
/// standalone remark under the primary one.
#[gen_stub_pyclass]
// `from_py_object`: a message is passed back in as a `Diagnostic` child.
#[pyclass(frozen, from_py_object)]
#[derive(Clone)]
pub struct DiagnosticMessage {
    pub(crate) data: OwnedDiagnosticMessage,
}

#[gen_stub_pymethods]
#[pymethods]
impl DiagnosticMessage {
    #[new]
    #[pyo3(signature = (message, *, span = None, kind = None))]
    fn new(
        py: Python<'_>,
        message: String,
        span: Option<Py<Span>>,
        kind: Option<DiagnosticMessageKind>,
    ) -> DiagnosticMessage {
        DiagnosticMessage {
            data: OwnedDiagnosticMessage {
                kind: kind.unwrap_or(DiagnosticMessageKind::Note).into(),
                message,
                snippet: snippet_of_span(py, span),
            },
        }
    }

    /// Whether this is an annotation or a note.
    #[getter]
    fn kind(&self) -> DiagnosticMessageKind {
        self.data.kind.into()
    }

    #[getter]
    fn message(&self) -> &str {
        &self.data.message
    }

    /// Where this message points, or `None` if it has no location.
    #[getter]
    fn location(&self) -> Option<Loc> {
        self.data.snippet.as_ref().map(loc_of_snippet)
    }

    /// The `include` chain that spliced `location`'s file in, nearest first;
    /// empty when the file was compiled directly.
    #[getter]
    fn includes(&self) -> Vec<Loc> {
        self.data
            .snippet
            .as_ref()
            .map_or_else(Vec::new, includes_of_snippet)
    }

    /// The source line(s) this message points at, as they appear in the file.
    #[getter]
    fn source(&self) -> Option<&str> {
        self.data
            .snippet
            .as_ref()
            .map(|snippet| snippet.file_content.as_str())
    }

    fn __str__(&self) -> String {
        let kind = self.kind().member_name();
        match self.location() {
            Some(location) => {
                format!(
                    "{}: {}: {}",
                    location.display_string(),
                    kind,
                    self.data.message
                )
            }
            None => format!("{}: {}", kind, self.data.message),
        }
    }

    fn __repr__(&self) -> String {
        format!("<DiagnosticMessage {}>", self.__str__())
    }
}

/// A diagnostic: a level, a message, the source it points at, and any child
/// messages.
///
/// Construct one to report a finding of your own against a model:
///
/// ```python
/// fpp.Diagnostic(
///     fpp.DiagnosticLevel.Error, "component is missing a command port",
///     span=component.node.span,
///     children=[fpp.DiagnosticMessage("declared here", span=other.node.span)],
/// )
/// ```
///
/// `str()` renders it the way the `fpp` compiler renders a diagnostic on the
/// console — source excerpt, carets, and all — while
/// `display` is the one-line form.
#[gen_stub_pyclass]
#[pyclass(frozen)]
pub struct Diagnostic {
    pub(crate) data: OwnedDiagnostic,
}

#[gen_stub_pymethods]
#[pymethods]
impl Diagnostic {
    #[new]
    #[pyo3(signature = (level, message, *, span = None, children = Vec::new()))]
    fn new(
        py: Python<'_>,
        level: DiagnosticLevel,
        message: String,
        span: Option<Py<Span>>,
        children: Vec<DiagnosticMessage>,
    ) -> Diagnostic {
        Diagnostic {
            data: OwnedDiagnostic {
                level: level.into(),
                message,
                snippet: snippet_of_span(py, span),
                children: children.into_iter().map(|child| child.data).collect(),
            },
        }
    }

    /// This diagnostic's severity.
    #[getter]
    fn level(&self) -> DiagnosticLevel {
        DiagnosticLevel::from(self.data.level)
    }

    #[getter]
    fn message(&self) -> &str {
        &self.data.message
    }

    /// Where this diagnostic points, or `None` if it has no location.
    #[getter]
    fn location(&self) -> Option<Loc> {
        self.data.snippet.as_ref().map(loc_of_snippet)
    }

    /// The `include` chain that spliced `location`'s file in, nearest first;
    /// empty when the file was compiled directly.
    #[getter]
    fn includes(&self) -> Vec<Loc> {
        self.data
            .snippet
            .as_ref()
            .map_or_else(Vec::new, includes_of_snippet)
    }

    /// The source line(s) this diagnostic points at, as they appear in the file.
    #[getter]
    fn source(&self) -> Option<&str> {
        self.data
            .snippet
            .as_ref()
            .map(|snippet| snippet.file_content.as_str())
    }

    /// The child messages, in the order they were added.
    #[getter]
    fn children(&self) -> Vec<DiagnosticMessage> {
        self.data
            .children
            .iter()
            .map(|child| DiagnosticMessage {
                data: child.clone(),
            })
            .collect()
    }

    /// This diagnostic as a one-line compiler message:
    /// `"path/to/file.fpp:12:5: error: cannot find type `Nope` in scope"`, or
    /// `"error: <message>"` when there is no location.
    ///
    /// The children are not part of it — see `render` for the full rendering.
    #[getter]
    fn display(&self) -> String {
        match self.location() {
            Some(location) => format!(
                "{}: {}: {}",
                location.display_string(),
                self.level().spelling(),
                self.data.message
            ),
            None => format!("{}: {}", self.level().spelling(), self.data.message),
        }
    }

    /// This diagnostic as the compiler renders it on the console: the message,
    /// the annotated source excerpt, and every child message.
    ///
    /// With `color`, the result carries ANSI escapes
    #[pyo3(signature = (*, color = false))]
    fn render(&self, color: bool) -> String {
        self.data.render(&fpp_errors::renderer(color))
    }

    fn __str__(&self) -> String {
        self.render(false)
    }

    fn __repr__(&self) -> String {
        format!("<Diagnostic {}>", self.display())
    }
}

impl Diagnostic {
    pub fn from_owned(diagnostic: &OwnedDiagnostic) -> Diagnostic {
        Diagnostic {
            data: diagnostic.clone(),
        }
    }
}
