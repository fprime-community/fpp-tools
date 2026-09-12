use crate::diagnostic::DiagnosticMessage;
use crate::file::SourceFile;
use crate::interface::with;
use crate::map::IdMap;
use crate::span::Span;
use crate::{BytePos, Diagnostic, DiagnosticMessageKind, Level, Node, Position};
use line_index::{LineCol, LineIndex};
use rustc_hash::FxHashSet;
use std::ops::Deref;
use std::sync::{Arc, Weak};

#[derive(Clone, Debug)]
pub struct SpanData {
    pub handle: usize,
    pub file: Weak<SourceFileData>,
    pub start: BytePos,
    pub length: BytePos,
    pub include_span: Option<Box<SpanData>>,
}

impl SpanData {
    pub fn snippet(&self) -> DiagnosticDataSnippet {
        self.file.upgrade().unwrap().snippet(self)
    }
}

#[derive(Clone, Debug, Default)]
pub struct GarbageCollectionSet {
    pub files: FxHashSet<usize>,
    pub spans: FxHashSet<usize>,
    pub nodes: FxHashSet<usize>,
}

impl GarbageCollectionSet {
    pub fn get_uri<E: DiagnosticEmitter>(
        &self,
        ctx: &CompilerContext<E>,
        uri: &str,
    ) -> Option<SourceFile> {
        for file in &self.files {
            if uri == ctx.files.get(*file).uri {
                return Some(SourceFile { handle: *file });
            }
        }

        None
    }

    pub fn start() {
        with(|ctx| ctx.garbage_collection_start())
    }

    pub fn finish() -> GarbageCollectionSet {
        with(|ctx| ctx.garbage_collection_finish())
    }

    pub fn cleanup(&self) {
        with(|ctx| ctx.garbage_collection_cleanup(self));
    }
}

pub struct SourceFileData {
    pub handle: usize,
    pub uri: String,
    pub content: String,
    pub lines: LineIndex,
    pub parent: Option<SourceFile>,
}

impl SourceFileData {
    fn new(
        handle: usize,
        uri: String,
        content: String,
        parent: Option<SourceFile>,
    ) -> SourceFileData {
        let lines = LineIndex::new(&content);

        SourceFileData {
            handle,
            uri,
            content,
            lines,
            parent,
        }
    }

    pub fn position(&self, offset: BytePos) -> Position {
        let raw_position = self.lines.line_col(offset.into());
        Position {
            pos: offset,
            line: raw_position.line,
            column: raw_position.col,
            source_file: SourceFile {
                handle: self.handle,
            },
        }
    }

    pub fn offset_of(&self, pos: LineCol) -> BytePos {
        self.lines.offset(pos).unwrap().into()
    }

    pub fn include_loc(&self, span: &SpanData) -> DiagnosticDataIncludeLocation {
        let pos = self.position(span.start);

        DiagnosticDataIncludeLocation {
            line: pos.line,
            column: pos.column,
            uri: self.uri.clone(),
        }
    }

    pub fn snippet(&self, span: &SpanData) -> DiagnosticDataSnippet {
        // Find the line start of the start/end
        let first_line_i = self.lines.line_col(span.start.into()).line;
        let last_line_i = self.lines.line_col((span.start + span.length).into()).line;

        let first_line = self.lines.line(first_line_i).unwrap();
        let last_line = self.lines.line(last_line_i).unwrap();
        let full_range = first_line.cover(last_line);

        let first: BytePos = first_line.start().into();

        // Collect all the include locations
        fn collect_include_locs(
            loc: &Option<Box<SpanData>>,
            out: &mut Vec<DiagnosticDataIncludeLocation>,
        ) {
            match loc {
                None => {}
                Some(loc) => {
                    out.push(loc.file.upgrade().unwrap().include_loc(loc));
                    collect_include_locs(&loc.include_span, out)
                }
            }
        }

        let mut include_spans = vec![];
        collect_include_locs(&span.include_span, &mut include_spans);

        DiagnosticDataSnippet {
            start: span.start - first,
            end: span.start + span.length - first,
            line_offset: first_line_i as usize,
            uri: self.uri.clone(),
            file_content: self.content[full_range.start().into()..full_range.end().into()].into(),
            include_spans,
        }
    }
}

impl From<&SourceFileData> for SourceFile {
    fn from(value: &SourceFileData) -> Self {
        SourceFile {
            handle: value.handle,
        }
    }
}

pub struct NodeData {
    pub span_handle: usize,
    pub pre_annotation: Vec<String>,
    pub post_annotation: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct DiagnosticDataIncludeLocation {
    pub line: u32,
    pub column: u32,
    pub uri: String,
}

#[derive(Clone, Debug)]
pub struct DiagnosticDataSnippet {
    pub start: BytePos,
    pub end: BytePos,
    pub line_offset: usize,
    pub uri: String,
    pub file_content: String,
    pub include_spans: Vec<DiagnosticDataIncludeLocation>,
}

#[derive(Debug)]
pub struct DiagnosticMessageData {
    pub kind: DiagnosticMessageKind,
    pub message: String,
    pub span: Option<SpanData>,
}

#[derive(Debug)]
pub struct DiagnosticData {
    pub level: Level,
    pub message: String,
    pub span: SpanData,
    pub children: Vec<DiagnosticMessageData>,
}

pub trait DiagnosticEmitter {
    fn emit(&mut self, diagnostic: DiagnosticData);
}

pub struct CompilerContext<E: DiagnosticEmitter> {
    spans: IdMap<SpanData>,
    files: IdMap<Arc<SourceFileData>>,
    nodes: IdMap<NodeData>,
    emitter: E,

    gc: Option<GarbageCollectionSet>,
}

impl<E: DiagnosticEmitter> CompilerContext<E> {
    pub fn new(emitter: E) -> CompilerContext<E> {
        CompilerContext {
            spans: Default::default(),
            files: Default::default(),
            nodes: Default::default(),
            emitter,

            gc: Default::default(),
        }
    }

    pub(crate) fn file_new(
        &mut self,
        uri: &str,
        content: String,
        parent: Option<SourceFile>,
    ) -> SourceFile {
        let handle = self.files.push_with(|handle| {
            Arc::new(SourceFileData::new(
                handle,
                uri.to_string(),
                content,
                parent,
            ))
        });

        self.gc.as_mut().map(|g| g.files.insert(handle));

        SourceFile { handle }
    }

    pub(crate) fn garbage_collection_start(&mut self) {
        assert!(self.gc.is_none(), "GC context is already set");
        self.gc = Some(GarbageCollectionSet::default());
    }

    pub(crate) fn garbage_collection_finish(&mut self) -> GarbageCollectionSet {
        match self.gc.take() {
            Some(context) => context,
            None => panic!("GC context is not set"),
        }
    }

    pub(crate) fn garbage_collection_cleanup(&mut self, gc: &GarbageCollectionSet) {
        self.files.extract_if(|k, _| gc.files.contains(k));
        self.spans.extract_if(|k, _| gc.spans.contains(k));
        self.nodes.extract_if(|k, _| gc.nodes.contains(k));
    }

    pub fn files(&self) -> impl Iterator<Item = &SourceFileData> {
        self.files.iter().map(|f| f.deref())
    }

    pub(crate) fn node_add(&mut self, span: &Span) -> Node {
        let handle = self.nodes.push(NodeData {
            span_handle: span.handle,
            pre_annotation: vec![],
            post_annotation: vec![],
        });

        self.gc.as_mut().map(|g| g.nodes.insert(handle));

        Node { handle }
    }

    pub(crate) fn span_add(
        &mut self,
        file: SourceFile,
        start: BytePos,
        length: BytePos,
        include_span: Option<Span>,
    ) -> Span {
        let file = self.files.get(file.handle);
        let include_span = include_span.map(|s| Box::new(self.span_get(&s).clone()));
        let handle = self.spans.push_with(|handle| SpanData {
            handle,
            file: Arc::downgrade(file),
            start,
            length,
            include_span,
        });

        self.gc.as_mut().map(|g| g.spans.insert(handle));

        Span { handle }
    }

    pub fn node_get(&self, node: &Node) -> &NodeData {
        self.nodes.get(node.handle)
    }

    /// Fallible variant of [`CompilerContext::node_get`]. Returns `None` when
    /// the node handle is no longer present in the context (for example, a
    /// symbol held by a stale analysis snapshot whose translation unit has
    /// already been garbage collected). Callers that may outlive the nodes they
    /// reference should prefer this over `node_get`.
    pub fn node_try_get(&self, node: &Node) -> Option<&NodeData> {
        self.nodes.try_get(node.handle)
    }

    pub(crate) fn node_get_mut(&mut self, node: &Node) -> &mut NodeData {
        self.nodes.get_mut(node.handle)
    }

    pub fn node_get_span(&self, node: &Node) -> Span {
        Span {
            handle: self.nodes.get(node.handle).span_handle,
        }
    }

    pub fn span_get(&self, span: &Span) -> &SpanData {
        self.spans.get(span.handle)
    }

    pub fn file_get(&self, file: &SourceFile) -> &SourceFileData {
        self.files.get(file.handle)
    }

    fn diagnostic_message_get(&self, diagnostic: DiagnosticMessage) -> DiagnosticMessageData {
        DiagnosticMessageData {
            message: diagnostic.message,
            kind: diagnostic.kind,
            span: diagnostic.span.map(|s| self.span_get(&s).clone()),
        }
    }

    fn diagnostic_get(&self, diagnostic: Diagnostic) -> DiagnosticData {
        DiagnosticData {
            level: diagnostic.level,
            message: diagnostic.msg,
            span: self.span_get(&diagnostic.span).clone(),
            children: diagnostic
                .children
                .into_iter()
                .map(|child| self.diagnostic_message_get(child))
                .collect(),
        }
    }

    pub(crate) fn diagnostic_emit(&mut self, diag: Diagnostic) {
        // Convert a standard diagnostic to a flattened diagnostic
        // Send to the emitter
        self.emitter.emit(self.diagnostic_get(diag));
    }
}
