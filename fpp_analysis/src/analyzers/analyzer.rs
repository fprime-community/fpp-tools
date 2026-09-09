use fpp_ast::{Node, Visitor};
use std::ops::ControlFlow;

/// A generic analysis visitor
pub trait Analyzer<'ast, V: Visitor<'ast>> {
    fn visit(&self, visitor: &V, a: &mut V::State, node: Node<'ast>) -> ControlFlow<V::Break>;
}
