use crate::Span;
use crate::interface::with;
use std::fmt::{Debug, Formatter};

/// A value with a location in the source
pub trait Spanned {
    /// Get the location
    fn span(&self) -> Span;
}

/// A value with the annotations of an AST node
pub trait Annotated {
    /// Get the pre-annotation lines, the `@` comments before the node
    fn pre_annotation(&self) -> Vec<String>;
    /// Get the post-annotation lines, the `@<` comments after the node
    fn post_annotation(&self) -> Vec<String>;
}

/// A node identifier
///
/// Nodes are ordered by allocation order, which is deterministic for a given
/// model. The order carries no meaning of its own; it is a stable tiebreak for
/// comparisons that would otherwise be ambiguous.
#[derive(Eq, Hash, PartialEq, Ord, PartialOrd, Copy, Clone)]
pub struct Node {
    pub(crate) handle: usize,
}

impl Node {
    /// Create a new node with a fresh identifier, located at `span`
    pub fn new(span: Span) -> Node {
        with(|w| w.node_add(&span))
    }

    /// Record the pre and post annotations of a node
    pub fn annotate(node: &Node, pre: Vec<String>, post: Vec<String>) {
        with(|w| w.node_add_annotation(node, pre, post))
    }
}

impl Spanned for Node {
    /// Get the span of an AST node. Panics if the node is not in the context.
    fn span(&self) -> Span {
        with(|w| w.node_span(self))
    }
}

impl Debug for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.span().fmt(f)
    }
}

impl Annotated for Node {
    fn pre_annotation(&self) -> Vec<String> {
        with(|w| w.node_pre_annotation(self))
    }

    fn post_annotation(&self) -> Vec<String> {
        with(|w| w.node_post_annotation(self))
    }
}
