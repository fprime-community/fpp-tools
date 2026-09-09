use line_index::LineIndex;

use crate::context::CompilerContext;
use crate::{
    BytePos, Diagnostic, DiagnosticEmitter, GarbageCollectionSet, Node, Position, SourceFile, Span,
};
use std::cell::{Cell, Ref, RefCell};

struct Container<'ctx, E: DiagnosticEmitter> {
    ctx: RefCell<&'ctx mut CompilerContext<E>>,
}

impl<'ctx, E: DiagnosticEmitter> Container<'ctx, E> {
    pub fn new(ctx: &'ctx mut CompilerContext<E>) -> Container<'ctx, E> {
        Container {
            ctx: RefCell::new(ctx),
        }
    }
}

impl<'ctx, E: DiagnosticEmitter> CompilerInterface for Container<'ctx, E> {
    fn node_add(&self, span: &Span) -> Node {
        self.ctx.borrow_mut().node_add(span)
    }

    fn node_span(&self, node: &Node) -> Span {
        self.ctx.borrow().node_get_span(node)
    }

    fn node_pre_annotation(&self, node: &Node) -> Vec<String> {
        self.ctx.borrow().node_get(node).pre_annotation.clone()
    }

    fn node_post_annotation(&self, node: &Node) -> Vec<String> {
        self.ctx.borrow().node_get(node).post_annotation.clone()
    }

    fn node_add_annotation(&self, node: &Node, pre: Vec<String>, post: Vec<String>) {
        let mut ctx = self.ctx.borrow_mut();
        let node = ctx.node_get_mut(node);
        node.pre_annotation = pre;
        node.post_annotation = post;
    }

    fn file_new(&self, uri: &str, content: String, parent: Option<SourceFile>) -> SourceFile {
        self.ctx.borrow_mut().file_new(uri, content, parent)
    }

    fn file_uri(&self, file: &SourceFile) -> String {
        self.ctx.borrow().file_get(file).uri.clone()
    }

    fn file_parent(&self, file: &SourceFile) -> Option<SourceFile> {
        self.ctx.borrow_mut().file_get(file).parent
    }

    fn file_content(&self, file: &SourceFile) -> Ref<'_, String> {
        // self.ctx.borrow().file_get(file).content.clone()
        let ctx = self.ctx.borrow();
        Ref::map(ctx, |c| &c.file_get(file).content)
    }

    fn file_lines(&self, file: &SourceFile) -> Ref<'_, LineIndex> {
        let ctx = self.ctx.borrow();
        Ref::map(ctx, |c| &c.file_get(file).lines)
    }

    fn file_len(&self, file: &SourceFile) -> usize {
        self.ctx.borrow().file_get(file).content.len()
    }

    fn span_add(
        &self,
        file: SourceFile,
        start: BytePos,
        length: BytePos,
        include_span: Option<Span>,
    ) -> Span {
        self.ctx
            .borrow_mut()
            .span_add(file, start, length, include_span)
    }

    fn span_start(&self, s: &Span) -> Position {
        let ctx = self.ctx.borrow();
        let data = ctx.span_get(s);
        data.file.upgrade().unwrap().position(data.start)
    }

    fn span_end(&self, s: &Span) -> Position {
        let ctx = self.ctx.borrow();
        let data = ctx.span_get(s);
        data.file
            .upgrade()
            .unwrap()
            .position(data.start + (data.length as BytePos))
    }

    fn span_len(&self, s: &Span) -> usize {
        let ctx = self.ctx.borrow();
        let data = ctx.span_get(s);
        data.length as usize
    }

    fn span_file(&self, s: &Span) -> SourceFile {
        let ctx = self.ctx.borrow();
        SourceFile {
            handle: ctx.span_get(s).file.upgrade().unwrap().handle,
        }
    }

    fn span_include_span(&self, s: &Span) -> Option<Span> {
        let ctx = self.ctx.borrow();
        Some(Span {
            handle: ctx.span_get(s).include_span.clone()?.handle,
        })
    }

    fn diagnostic_emit(&self, diag: Diagnostic) {
        self.ctx.borrow_mut().diagnostic_emit(diag)
    }

    fn garbage_collection_start(&self) {
        self.ctx.borrow_mut().garbage_collection_start();
    }

    fn garbage_collection_finish(&self) -> GarbageCollectionSet {
        self.ctx.borrow_mut().garbage_collection_finish()
    }

    fn garbage_collection_cleanup(&self, gc: &GarbageCollectionSet) {
        self.ctx.borrow_mut().garbage_collection_cleanup(gc);
    }
}

/// A read-only [`CompilerInterface`] backed by a shared `&CompilerContext`.
///
/// Used by [`run_ref`] to make the context available on the current thread for
/// read-only reflection (spans, annotations, port lookups) without requiring a
/// mutable borrow. Any mutating interface method panics: callers running under
/// `run_ref` must not create nodes/spans/files or emit diagnostics.
struct RefContainer<'ctx, E: DiagnosticEmitter> {
    ctx: RefCell<&'ctx CompilerContext<E>>,
}

impl<'ctx, E: DiagnosticEmitter> RefContainer<'ctx, E> {
    fn new(ctx: &'ctx CompilerContext<E>) -> RefContainer<'ctx, E> {
        RefContainer {
            ctx: RefCell::new(ctx),
        }
    }
}

impl<E: DiagnosticEmitter> CompilerInterface for RefContainer<'_, E> {
    fn node_add(&self, _span: &Span) -> Node {
        panic!("cannot add a node under a read-only compiler context")
    }

    fn node_span(&self, node: &Node) -> Span {
        self.ctx.borrow().node_get_span(node)
    }

    fn node_pre_annotation(&self, node: &Node) -> Vec<String> {
        self.ctx.borrow().node_get(node).pre_annotation.clone()
    }

    fn node_post_annotation(&self, node: &Node) -> Vec<String> {
        self.ctx.borrow().node_get(node).post_annotation.clone()
    }

    fn node_add_annotation(&self, _node: &Node, _pre: Vec<String>, _post: Vec<String>) {
        panic!("cannot annotate a node under a read-only compiler context")
    }

    fn file_new(&self, _uri: &str, _content: String, _parent: Option<SourceFile>) -> SourceFile {
        panic!("cannot create a file under a read-only compiler context")
    }

    fn file_uri(&self, file: &SourceFile) -> String {
        self.ctx.borrow().file_get(file).uri.clone()
    }

    fn file_parent(&self, file: &SourceFile) -> Option<SourceFile> {
        self.ctx.borrow().file_get(file).parent
    }

    fn file_content(&self, file: &SourceFile) -> Ref<'_, String> {
        let ctx = self.ctx.borrow();
        Ref::map(ctx, |c| &c.file_get(file).content)
    }

    fn file_lines(&self, file: &SourceFile) -> Ref<'_, LineIndex> {
        let ctx = self.ctx.borrow();
        Ref::map(ctx, |c| &c.file_get(file).lines)
    }

    fn file_len(&self, file: &SourceFile) -> usize {
        self.ctx.borrow().file_get(file).content.len()
    }

    fn span_add(
        &self,
        _file: SourceFile,
        _start: BytePos,
        _length: BytePos,
        _include_span: Option<Span>,
    ) -> Span {
        panic!("cannot add a span under a read-only compiler context")
    }

    fn span_start(&self, s: &Span) -> Position {
        let ctx = self.ctx.borrow();
        let data = ctx.span_get(s);
        data.file.upgrade().unwrap().position(data.start)
    }

    fn span_end(&self, s: &Span) -> Position {
        let ctx = self.ctx.borrow();
        let data = ctx.span_get(s);
        data.file
            .upgrade()
            .unwrap()
            .position(data.start + (data.length as BytePos))
    }

    fn span_len(&self, s: &Span) -> usize {
        let ctx = self.ctx.borrow();
        ctx.span_get(s).length as usize
    }

    fn span_file(&self, s: &Span) -> SourceFile {
        let ctx = self.ctx.borrow();
        SourceFile {
            handle: ctx.span_get(s).file.upgrade().unwrap().handle,
        }
    }

    fn span_include_span(&self, s: &Span) -> Option<Span> {
        let ctx = self.ctx.borrow();
        Some(Span {
            handle: ctx.span_get(s).include_span.clone()?.handle,
        })
    }

    fn diagnostic_emit(&self, _diag: Diagnostic) {
        panic!("cannot emit a diagnostic under a read-only compiler context")
    }

    fn garbage_collection_start(&self) {
        panic!("cannot run garbage collection under a read-only compiler context")
    }

    fn garbage_collection_finish(&self) -> GarbageCollectionSet {
        panic!("cannot run garbage collection under a read-only compiler context")
    }

    fn garbage_collection_cleanup(&self, _gc: &GarbageCollectionSet) {
        panic!("cannot run garbage collection under a read-only compiler context")
    }
}

pub(crate) trait CompilerInterface {
    /// Ast Node related functions
    fn node_add(&self, span: &Span) -> Node;
    fn node_span(&self, node: &Node) -> Span;
    fn node_pre_annotation(&self, node: &Node) -> Vec<String>;
    fn node_post_annotation(&self, node: &Node) -> Vec<String>;
    fn node_add_annotation(&self, node: &Node, pre: Vec<String>, post: Vec<String>);

    /// Source file related functions
    fn file_new(&self, uri: &str, content: String, parent: Option<SourceFile>) -> SourceFile;
    fn file_uri(&self, file: &SourceFile) -> String;
    fn file_parent(&self, file: &SourceFile) -> Option<SourceFile>;
    fn file_content(&self, file: &SourceFile) -> Ref<'_, String>;
    fn file_lines(&self, file: &SourceFile) -> Ref<'_, LineIndex>;
    fn file_len(&self, file: &SourceFile) -> usize;

    /// Span related functions
    fn span_add(
        &self,
        file: SourceFile,
        start: BytePos,
        length: BytePos,
        include_span: Option<Span>,
    ) -> Span;
    fn span_start(&self, s: &Span) -> Position;
    fn span_end(&self, s: &Span) -> Position;
    fn span_len(&self, s: &Span) -> usize;
    fn span_file(&self, s: &Span) -> SourceFile;
    fn span_include_span(&self, s: &Span) -> Option<Span>;

    /// Diagnostic related functions
    fn diagnostic_emit(&self, diag: Diagnostic);

    /// Garbage collection related functions
    fn garbage_collection_start(&self);
    fn garbage_collection_finish(&self) -> GarbageCollectionSet;
    fn garbage_collection_cleanup(&self, gc: &GarbageCollectionSet);
}

// A thread local variable that stores a pointer to [`CompilerInterface`].
scoped_tls::scoped_thread_local!(static TLV: Cell<*const ()>);

/// Run the compiler under a closure with a compiler context
///
/// # Arguments
///
/// * `ctx`: Context to attach to the core compiler
/// * `f`: Function closure to run
pub fn run<F, T, E>(ctx: &mut CompilerContext<E>, f: F) -> T
where
    F: FnOnce() -> T,
    E: DiagnosticEmitter,
{
    let container = Container::new(ctx);
    run1(&container, f)
}

/// Run a closure with read-only access to a compiler context.
///
/// Like [`run`], but takes a shared `&CompilerContext`, so it can be called
/// while the context is otherwise borrowed immutably (e.g. from an LSP request
/// handler holding `&GlobalState`). This makes reflection APIs that read the
/// context through the thread-local (spans, annotations, and analysis helpers
/// that call [`Spanned::span`]) usable without a mutable borrow.
///
/// The closure must not mutate the context: creating nodes/spans/files or
/// emitting diagnostics will panic.
pub fn run_ref<F, T, E>(ctx: &CompilerContext<E>, f: F) -> T
where
    F: FnOnce() -> T,
    E: DiagnosticEmitter,
{
    let container = RefContainer::new(ctx);
    run1(&container, f)
}

fn run1<F, T>(interface: &dyn CompilerInterface, f: F) -> T
where
    F: FnOnce() -> T,
{
    if TLV.is_set() {
        panic!("fpp_core already running");
    }

    let ptr: *const () = (&raw const interface) as _;
    TLV.set(&Cell::new(ptr), f)
}

/// Execute the given function with access the [`CompilerInterface`].
///
/// I.e., This function will load the current interface and calls a function with it.
/// Do not nest these, as that will ICE.
pub(crate) fn with<R>(f: impl FnOnce(&'static dyn CompilerInterface) -> R) -> R {
    assert!(TLV.is_set());
    TLV.with(|tlv| {
        let ptr = tlv.get();
        assert!(!ptr.is_null());
        f(unsafe { *(ptr as *const &dyn CompilerInterface) })
    })
}
