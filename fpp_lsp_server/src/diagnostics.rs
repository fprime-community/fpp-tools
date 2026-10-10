use fpp_core::{DiagnosticData, DiagnosticEmitter};
use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, Position, Range, Uri,
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    str::FromStr,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug)]
pub struct LspDiagnostic {
    id: usize,
    diagnostic: Diagnostic,
}

#[derive(Default)]
struct LspDiagnosticsEmitterInner {
    next_id: usize,
    diagnostics: FxHashMap<String, Vec<LspDiagnostic>>,
    garbage_collection_set: Option<FxHashSet<usize>>,
}

#[derive(Clone, Default)]
pub struct LspDiagnosticsEmitter(Arc<Mutex<LspDiagnosticsEmitterInner>>);

impl LspDiagnosticsEmitterInner {
    pub fn clear(&mut self) {
        self.diagnostics.clear();
    }

    /// Returns all diagnostics for a specific URI
    pub fn get(&self, uri: &str) -> Vec<Diagnostic> {
        self.diagnostics
            .get(uri)
            .map_or_else(std::vec::Vec::new, |v| v.clone())
            .into_iter()
            .map(|d| d.diagnostic)
            .collect()
    }
}

impl LspDiagnosticsEmitter {
    pub fn clear(&mut self) {
        self.0.lock().unwrap().clear();
    }

    /// Returns all diagnostics for a specific URI
    pub fn get(&self, uri: &str) -> Vec<Diagnostic> {
        self.0.lock().unwrap().get(uri)
    }

    /// Snapshot of every URI's diagnostics. Used to push diagnostics to the client
    /// after analysis so it never has to pull (and never sees a stale window).
    pub fn all(&self) -> FxHashMap<String, Vec<Diagnostic>> {
        let state = self.0.lock().unwrap();
        state
            .diagnostics
            .iter()
            .map(|(uri, diagnostics)| {
                (
                    uri.clone(),
                    diagnostics.iter().map(|d| d.diagnostic.clone()).collect(),
                )
            })
            .collect()
    }

    /// Start tracking all diagnostics
    pub fn start_garbage_collection(&self) {
        let mut state = self.0.lock().unwrap();
        assert!(state.garbage_collection_set.is_none());
        state.garbage_collection_set = Some(FxHashSet::default());
    }

    pub fn finish_garbage_collection(&self) -> FxHashSet<usize> {
        let mut state = self.0.lock().unwrap();
        let gc = state.garbage_collection_set.take();
        gc.expect("diagnostic garbage collection was not set")
    }

    pub fn cleanup_garbage_collection(&self, set: &FxHashSet<usize>) {
        let mut state = self.0.lock().unwrap();
        state.diagnostics.retain(|_, diagnostics| {
            diagnostics.retain(|diagnostic| !set.contains(&diagnostic.id));
            !diagnostics.is_empty()
        })
    }
}

fn span_data_to_range(span: &fpp_core::SpanData) -> Range {
    let file = span.file.upgrade().unwrap();
    let start = file.lines.line_col(span.start.into());
    let end = file.lines.line_col((span.start + span.length).into());

    Range::new(
        Position::new(start.line, start.col),
        Position::new(end.line, end.col),
    )
}

/// The chain of template expansion specifiers that copied `span` here, as
/// related information.
///
/// A template body is analyzed once per expansion, so one line of a template body
/// can carry one diagnostic per expansion of that template. Those diagnostics have
/// the same range and, when the arguments differ only in what they name, the same
/// message: without the expansion point they cannot be told apart in the editor.
fn expansion_related_information(
    span: &fpp_core::SpanData,
) -> impl Iterator<Item = DiagnosticRelatedInformation> {
    let mut current = span.expand_span.clone();
    std::iter::from_fn(move || {
        let expand = current.take()?;
        let file = expand.file.upgrade().unwrap();
        current = expand.expand_span.clone();
        Some(DiagnosticRelatedInformation {
            location: Location::new(
                Uri::from_str(&file.uri).unwrap(),
                span_data_to_range(&expand),
            ),
            // The location carries the file and position that the command line
            // renderer prints after this phrase.
            message: "expanded at".to_string(),
        })
    })
}

fn diagnostic_level_to_severity(level: fpp_core::Level) -> DiagnosticSeverity {
    match level {
        fpp_core::Level::Error => DiagnosticSeverity::ERROR,
        fpp_core::Level::Warning => DiagnosticSeverity::WARNING,
        fpp_core::Level::Note => DiagnosticSeverity::INFORMATION,
        fpp_core::Level::Help => DiagnosticSeverity::HINT,
        _ => DiagnosticSeverity::INFORMATION,
    }
}

impl DiagnosticEmitter for LspDiagnosticsEmitter {
    fn emit(&mut self, diagnostic: DiagnosticData) {
        let file = diagnostic.span.file.upgrade().unwrap();
        let uri = Uri::from_str(&file.uri).unwrap();
        let range = span_data_to_range(&diagnostic.span);

        let uri_c = uri.clone();
        let expansions = expansion_related_information(&diagnostic.span);
        let related_information = Some(
            diagnostic
                .children
                .into_iter()
                .map(|sub| {
                    let location = match sub.span {
                        None => Location::new(uri_c.clone(), range),
                        Some(span) => Location::new(
                            Uri::from_str(&span.file.upgrade().unwrap().uri).unwrap(),
                            span_data_to_range(&span),
                        ),
                    };

                    DiagnosticRelatedInformation {
                        location,
                        message: sub.message,
                    }
                })
                .chain(expansions)
                .collect(),
        );

        let mut state = self.0.lock().unwrap();
        let id = state.next_id;

        let lsp_diagnostic = LspDiagnostic {
            id,
            diagnostic: Diagnostic {
                range,
                severity: Some(diagnostic_level_to_severity(diagnostic.level)),
                source: Some("fpp".to_owned()),
                message: diagnostic.message,
                related_information,
                ..Diagnostic::default()
            },
        };

        match &mut state.garbage_collection_set {
            None => {}
            Some(gc) => {
                gc.insert(id);
            }
        }

        state.next_id += 1;

        match state.diagnostics.get_mut(&file.uri) {
            None => {
                state
                    .diagnostics
                    .insert(file.uri.clone(), vec![lsp_diagnostic]);
            }
            Some(c) => c.push(lsp_diagnostic),
        }
    }
}
