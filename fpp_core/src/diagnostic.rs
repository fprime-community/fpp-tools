use crate::interface::with;
use crate::{Span, Spanned};

/// An enum representing a diagnostic level.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Level {
    /// An error.
    Error,
    /// A warning.
    Warning,
    /// A note.
    Note,
    /// A help message.
    Help,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticMessageKind {
    Primary,
    Note,
}

#[derive(Clone, Debug)]
pub(crate) struct DiagnosticMessage {
    pub kind: DiagnosticMessageKind,
    pub message: String,
    pub span: Option<Span>,
}

/// A structure representing a diagnostic message and associated children
/// messages.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub(crate) level: Level,
    pub(crate) msg: String,
    pub(crate) span: Span,
    pub(crate) children: Vec<DiagnosticMessage>,
}

macro_rules! diagnostic_child_methods {
    ($spanned:ident, $regular:ident, $kind:expr) => {
        #[doc = concat!("Adds a new child diagnostics message to `self` with the [`",
                                stringify!($kind), "`] level, and the given `span` and `message`.")]
        pub fn $spanned<S, T>(mut self, span: S, message: T) -> Diagnostic
        where
            S: Spanned,
            T: Into<String>,
        {
            self.children.push(DiagnosticMessage {
                kind: $kind,
                message: message.into(),
                span: Some(span.span()),
            });
            self
        }

        #[doc = concat!("Adds a new child diagnostic message to `self` with the [`",
                                stringify!($kind), "`] level, and the given `message`.")]
        pub fn $regular<T: Into<String>>(mut self, message: T) -> Diagnostic {
            self.children.push(DiagnosticMessage {
                kind: $kind,
                message: message.into(),
                span: None,
            });
            self
        }
    };
}

impl Diagnostic {
    /// Creates a new diagnostic with the given `span`, `level` and `message`
    pub fn new<S, T>(span: S, level: Level, message: T) -> Diagnostic
    where
        S: Spanned,
        T: Into<String>,
    {
        Diagnostic {
            level,
            msg: message.into(),
            span: span.span(),
            children: vec![],
        }
    }

    diagnostic_child_methods!(span_annotation, annotation, DiagnosticMessageKind::Primary);
    diagnostic_child_methods!(span_note, note, DiagnosticMessageKind::Note);

    /// Emit the diagnostic.
    pub fn emit(self) {
        with(|w| w.diagnostic_emit(self));
    }
}
