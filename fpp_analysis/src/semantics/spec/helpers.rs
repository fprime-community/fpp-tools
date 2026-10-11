//! General helpers

use fpp_ast::{Expr, ExprKind, Name};
use rustc_hash::FxHashMap as HashMap;
use std::cell::RefCell;

thread_local! {
    /// Memoizes `id -> Node` so that repeated builder calls with the same integer
    /// id return the same node, giving each id a stable node identity.
    static NODE_BY_ID: RefCell<HashMap<u64, fpp_core::Node>> = RefCell::new(HashMap::default());
    /// A single shared source file / span for all synthesized nodes.
    static TEST_SPAN: RefCell<Option<fpp_core::Span>> = const { RefCell::new(None) };
}

/// Runs `f` inside a fresh compiler context with the node-id memo reset.
///
/// All type/value builders must be called inside this closure.
pub fn with_test_ctx<T>(f: impl FnOnce() -> T) -> T {
    let mut buf = vec![];
    let mut ctx = fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut buf));
    fpp_core::run(&mut ctx, || {
        NODE_BY_ID.with(|m| m.borrow_mut().clear());
        let src = fpp_core::SourceFile::new("test", "test".to_string());
        let span = fpp_core::Span::new(src, 0, 1, None);
        TEST_SPAN.with(|s| *s.borrow_mut() = Some(span));
        f()
    })
}

/// A synthesized span shared by all test nodes.
fn test_span() -> fpp_core::Span {
    TEST_SPAN.with(|s| s.borrow().expect("with_test_ctx not active"))
}

/// Returns the memoized node for `id`, creating it on first use. Same `id`
/// always yields the same [`fpp_core::Node`].
pub fn node_for(id: u64) -> fpp_core::Node {
    NODE_BY_ID.with(|m| {
        *m.borrow_mut()
            .entry(id)
            .or_insert_with(|| fpp_core::Node::new(test_span()))
    })
}

/// A name whose `node_id` is memoized by `id`.
pub fn name_with_id(data: &str, id: u64) -> Name {
    Name {
        data: data.to_string(),
        node_id: node_for(id),
    }
}

/// A literal-int expression node, memoized by `id`.
pub fn expr_lit_int(value: &str, id: u64) -> Expr {
    Expr {
        kind: ExprKind::LiteralInt(value.to_string()),
        node_id: node_for(id),
    }
}

/// The pair `(x, x)`
pub fn duplicate<T: Clone>(x: T) -> (T, T) {
    (x.clone(), x)
}
