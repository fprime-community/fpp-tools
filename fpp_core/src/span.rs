use crate::diagnostic::Diagnostic;
use crate::diagnostic::Level;
use crate::file::SourceFile;
use crate::interface::with;
use crate::{BytePos, Spanned};
use std::fmt::{Debug, Display, Formatter};

/// An opaque handle to a span recorded in the compiler context.
///
/// Every derived trait here compares the raw handle, which needs no live context
/// (unlike [`Span::start`] / [`Span::file`], which resolve through it). Handles are
/// allocated, not interned, so two spans over the same text are distinct — and the
/// `Ord` is allocation order, which is not source order. Order spans for output
/// with something position-based (e.g. `fpp_analysis::semantics::cmp_span`); use
/// this `Ord` only as a tie-breaker that keeps `cmp` consistent with `==`.
#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    pub(crate) handle: usize,
}

impl Debug for Span {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let start = self.start();
        let end = self.end();

        f.debug_struct("Span")
            .field("start", &start)
            .field("end", &end)
            .finish()
    }
}

macro_rules! diagnostic_method {
    ($name:ident, $level:expr) => {
        /// Creates a new `Diagnostic` with the given `message` at the span
        /// `self`.
        pub fn $name<T: Into<String>>(self, message: T) -> Diagnostic {
            Diagnostic::new(self, $level, message)
        }
    };
}

impl Span {
    pub fn new(
        file: SourceFile,
        start: BytePos,
        length: BytePos,
        include_span: Option<Span>,
    ) -> Span {
        with(|w| w.span_add(file, start, length, include_span))
    }

    /// Gets the start position of the span
    pub fn start(&self) -> Position {
        with(|w| w.span_start(self))
    }

    /// Creates an empty span pointing to directly after this span.
    pub fn end(&self) -> Position {
        with(|w| w.span_end(self))
    }

    pub fn len(&self) -> usize {
        with(|w| w.span_len(self))
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The path to the source file in which this span occurs, for display purposes.
    pub fn file(&self) -> SourceFile {
        with(|w| w.span_file(self))
    }

    /// Span of the include specifier that brought this span in
    pub fn including_span(&self) -> Option<Span> {
        with(|w| w.span_include_span(self))
    }

    diagnostic_method!(error, Level::Error);
    diagnostic_method!(warning, Level::Warning);
    diagnostic_method!(note, Level::Note);
    diagnostic_method!(help, Level::Help);
}

impl Spanned for Span {
    fn span(&self) -> Span {
        *self
    }
}

pub struct Position {
    pub(crate) pos: BytePos,
    pub(crate) line: u32,
    pub(crate) column: u32,
    pub(crate) source_file: SourceFile,
}

impl Position {
    pub fn pos(&self) -> BytePos {
        self.pos
    }

    /// Get the zero indexed line number at this position in the source file
    pub fn line(&self) -> u32 {
        self.line
    }

    /// Get the zero indexed column number at this position in the source file
    pub fn column(&self) -> u32 {
        self.column
    }
}

impl Display for Position {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{}:{}:{}",
            self.source_file,
            self.line + 1,
            self.column + 1
        ))
    }
}

impl Debug for Position {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{}:{}:{}",
            self.source_file,
            self.line + 1,
            self.column + 1
        ))
    }
}
