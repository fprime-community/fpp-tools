use crate::diagnostic::Diagnostic;
use crate::diagnostic::Level;
use crate::file::{File, SourceFile, normalize};
use crate::interface::with;
use crate::{BytePos, Spanned};
use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
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
        with(|w| w.span_add(file, start, length, include_span, None))
    }

    /// Creates a span that is a copy of the source text at `self`, made at the
    /// point of the template expansion specifier spanning `expand_span`.
    pub fn expanded_at(&self, expand_span: Span) -> Span {
        let start = self.start();
        with(|w| {
            w.span_add(
                w.span_file(self),
                start.pos(),
                self.len() as BytePos,
                w.span_include_span(self),
                Some(expand_span),
            )
        })
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

    /// Span of the template expansion specifier that copied this span here
    pub fn expanding_span(&self) -> Option<Span> {
        with(|w| w.span_expand_span(self))
    }

    /// Get the location of the associated translation unit
    ///
    /// A copy made by a template expansion belongs to the translation unit of the
    /// expansion specifier, so the expansion chain is followed before the include
    /// chain.
    pub fn tu_location(&self) -> Span {
        match (self.expanding_span(), self.including_span()) {
            (None, None) => *self,
            (Some(span), _) => span.tu_location(),
            (_, Some(span)) => span.tu_location(),
        }
    }

    /// Get the directory path associated with the location
    ///
    /// For standard input, this is the current working directory.
    pub fn get_dir_path(&self) -> PathBuf {
        let uri = self.file().uri();
        if uri == "<stdin>" {
            File::get_path("")
        } else {
            let path = File::get_path(&uri);
            path.parent().map(Path::to_path_buf).unwrap_or(path)
        }
    }

    /// Resolve a path relative to the path of this location
    ///
    /// The result is normalized lexically, without reading the file system.
    pub fn get_relative_path(&self, path: &str) -> PathBuf {
        normalize(&self.get_dir_path().join(path))
    }

    /// Compare two locations: by file, then by line and column, then by the
    /// location where each is included, then by the location where each is
    /// expanded
    pub fn compare(&self, other: &Span) -> Ordering {
        let file_compare = self.file().to_string().cmp(&other.file().to_string());
        if file_compare != Ordering::Equal {
            return file_compare;
        }
        let (start, other_start) = (self.start(), other.start());
        start
            .line()
            .cmp(&other_start.line())
            .then(start.column().cmp(&other_start.column()))
            .then_with(|| Span::compare_opt(self.including_span(), other.including_span()))
            .then_with(|| Span::compare_opt(self.expanding_span(), other.expanding_span()))
    }

    /// Compare two optional locations, ordering None before Some
    pub fn compare_opt(span1: Option<Span>, span2: Option<Span>) -> Ordering {
        match (span1, span2) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(s1), Some(s2)) => s1.compare(&s2),
        }
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

#[cfg(test)]
mod tests {
    use crate::{CompilerContext, DiagnosticData, DiagnosticEmitter, File, SourceFile, Span};
    use std::cmp::Ordering;
    use std::path::PathBuf;

    struct NoEmitter;

    impl DiagnosticEmitter for NoEmitter {
        fn emit(&mut self, _diagnostic: DiagnosticData) {}
    }

    fn run<T>(f: impl FnOnce() -> T) -> T {
        crate::run(&mut CompilerContext::new(NoEmitter), f)
    }

    /// A span of `len` bytes at byte `start` of a file with the given URI and two
    /// lines of text
    fn span_in(uri: &str, start: u32, len: u32, include_span: Option<Span>) -> Span {
        let file = SourceFile::new(uri, "constant a = 0\nconstant b = 1\n".to_string());
        Span::new(file, start, len, include_span)
    }

    #[test]
    fn tu_location_follows_the_expansion_chain_then_the_include_chain() {
        run(|| {
            let tu = span_in("/m/main.fpp", 0, 10, None);
            assert_eq!(tu.tu_location(), tu);

            let included = span_in("/m/a.fppi", 0, 5, Some(tu));
            assert_eq!(included.tu_location(), tu);
            let nested = span_in("/m/b.fppi", 0, 5, Some(included));
            assert_eq!(nested.tu_location(), tu);

            // A copy of the included text made by an expansion in another unit
            let expansion = span_in("/m/other.fpp", 15, 10, None);
            let copy = included.expanded_at(expansion);
            assert_eq!(copy.tu_location(), expansion);
        });
    }

    #[test]
    fn get_dir_path_is_the_directory_of_the_file() {
        run(|| {
            let span = span_in("/a/b/c.fpp", 0, 1, None);
            assert_eq!(span.get_dir_path(), PathBuf::from("/a/b"));

            let relative = span_in("x/./y.fpp", 0, 1, None);
            assert_eq!(relative.get_dir_path(), File::get_path("x"));

            let stdin = span_in("<stdin>", 0, 1, None);
            assert_eq!(stdin.get_dir_path(), std::env::current_dir().unwrap());
        });
    }

    #[test]
    fn get_relative_path_joins_and_normalizes_lexically() {
        run(|| {
            let span = span_in("/a/b/c.fpp", 0, 1, None);
            assert_eq!(span.get_relative_path("d.fpp"), PathBuf::from("/a/b/d.fpp"));
            assert_eq!(
                span.get_relative_path("./d/../e.fpp"),
                PathBuf::from("/a/b/e.fpp")
            );
            assert_eq!(
                span.get_relative_path("../../../../f.fpp"),
                PathBuf::from("/f.fpp")
            );
            // The directories in the path are not read
            assert_eq!(
                span.get_relative_path("nonexistent/../g.fppi"),
                PathBuf::from("/a/b/g.fppi")
            );
            assert_eq!(
                span.get_relative_path("/abs/h.fpp"),
                PathBuf::from("/abs/h.fpp")
            );
        });
    }

    #[test]
    fn normalize_keeps_a_leading_parent_of_a_relative_path() {
        use crate::file::normalize;
        assert_eq!(normalize("../../a".as_ref()), PathBuf::from("../../a"));
        assert_eq!(normalize("a/../..".as_ref()), PathBuf::from(".."));
        assert_eq!(normalize("a/./b/..".as_ref()), PathBuf::from("a"));
        assert_eq!(normalize("a/..".as_ref()), PathBuf::from(""));
        assert_eq!(normalize("/../a".as_ref()), PathBuf::from("/a"));
    }

    #[test]
    fn compare_orders_by_file_then_position_then_include_then_expansion() {
        run(|| {
            let a_line_1 = span_in("/m/a.fpp", 9, 1, None);
            let a_line_1_later = span_in("/m/a.fpp", 13, 1, None);
            let a_line_2 = span_in("/m/a.fpp", 15, 1, None);
            let b = span_in("/m/b.fpp", 0, 1, None);

            assert_eq!(a_line_1.compare(&a_line_1), Ordering::Equal);
            assert_eq!(a_line_1.compare(&a_line_1_later), Ordering::Less);
            assert_eq!(a_line_1_later.compare(&a_line_2), Ordering::Less);
            assert_eq!(a_line_2.compare(&b), Ordering::Less);
            assert_eq!(b.compare(&a_line_1), Ordering::Greater);

            // The same text, included at two places
            let included_early = span_in("/m/i.fppi", 0, 1, Some(a_line_1));
            let included_late = span_in("/m/i.fppi", 0, 1, Some(a_line_2));
            let not_included = span_in("/m/i.fppi", 0, 1, None);
            assert_eq!(not_included.compare(&included_early), Ordering::Less);
            assert_eq!(included_early.compare(&included_late), Ordering::Less);

            // The same text, copied by two expansions
            let copy_early = not_included.expanded_at(a_line_1);
            let copy_late = not_included.expanded_at(a_line_2);
            assert_eq!(not_included.compare(&copy_early), Ordering::Less);
            assert_eq!(copy_early.compare(&copy_late), Ordering::Less);
        });
    }

    #[test]
    fn compare_opt_orders_none_first() {
        run(|| {
            let span = span_in("/m/a.fpp", 0, 1, None);
            assert_eq!(Span::compare_opt(None, None), Ordering::Equal);
            assert_eq!(Span::compare_opt(None, Some(span)), Ordering::Less);
            assert_eq!(Span::compare_opt(Some(span), None), Ordering::Greater);
            assert_eq!(Span::compare_opt(Some(span), Some(span)), Ordering::Equal);
        });
    }
}
