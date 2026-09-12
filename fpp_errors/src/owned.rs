//! An owned, context-free copy of a [`DiagnosticData`].
//!
//! `DiagnosticData` borrows the compiler context: its spans hold a `Weak`
//! reference to the source file, so resolving one after the context is gone
//! panics. [`OwnedDiagnostic`] resolves every span to its
//! [`DiagnosticDataSnippet`] up front — the covered source lines, the byte range
//! inside them, and the include chain — so it can be stored, moved between
//! threads, and rendered later.
//!
//! Rendering goes through the same group builder the emitters use, so an owned
//! diagnostic renders byte-for-byte like the one the console printed.

use crate::snippet::diagnostic_to_snippet_group;
use annotate_snippets::Renderer;
use annotate_snippets::renderer::DecorStyle;
use fpp_core::{DiagnosticData, DiagnosticDataSnippet, DiagnosticMessageKind, Level};

/// A diagnostic detached from the compiler context that produced it.
#[derive(Clone, Debug)]
pub struct OwnedDiagnostic {
    pub level: Level,
    pub message: String,
    /// The primary source location. `None` for a diagnostic with no location —
    /// one built by hand rather than emitted by the compiler.
    pub snippet: Option<DiagnosticDataSnippet>,
    pub children: Vec<OwnedDiagnosticMessage>,
}

/// A child message of an [`OwnedDiagnostic`]: a further annotated span, or a
/// standalone note.
#[derive(Clone, Debug)]
pub struct OwnedDiagnosticMessage {
    pub kind: DiagnosticMessageKind,
    pub message: String,
    pub snippet: Option<DiagnosticDataSnippet>,
}

impl From<&DiagnosticData> for OwnedDiagnostic {
    fn from(diagnostic: &DiagnosticData) -> OwnedDiagnostic {
        OwnedDiagnostic {
            level: diagnostic.level,
            message: diagnostic.message.clone(),
            snippet: Some(diagnostic.span.snippet()),
            children: diagnostic
                .children
                .iter()
                .map(|child| OwnedDiagnosticMessage {
                    kind: child.kind.clone(),
                    message: child.message.clone(),
                    snippet: child.span.as_ref().map(|span| span.snippet()),
                })
                .collect(),
        }
    }
}

impl OwnedDiagnostic {
    /// Render this diagnostic the way an emitter would, without the trailing
    /// blank line the emitters add between diagnostics.
    pub fn render(&self, renderer: &Renderer) -> String {
        renderer.render(&[diagnostic_to_snippet_group(self)])
    }
}

/// The renderer the emitters use: ASCII decorations, with ANSI styling when
/// `color`.
pub fn renderer(color: bool) -> Renderer {
    let renderer = if color {
        Renderer::styled()
    } else {
        Renderer::plain()
    };
    renderer.decor_style(DecorStyle::Ascii)
}
