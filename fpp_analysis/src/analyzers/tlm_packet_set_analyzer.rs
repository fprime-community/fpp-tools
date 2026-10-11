use crate::Analysis;
use crate::analyzers::analyzer::{Analyzer, DefaultAnalyzer};
use fpp_ast::{Node, Visitable, Visitor};
use std::ops::ControlFlow;

/// Analyze telemetry packet set members
#[derive(Default)]
pub struct TlmPacketSetAnalyzer<A = DefaultAnalyzer> {
    super_: A,
}

impl<'ast, V, A> Analyzer<'ast, V> for TlmPacketSetAnalyzer<A>
where
    V: Visitor<'ast, State = Analysis>,
    A: Analyzer<'ast, V>,
{
    fn visit(&self, visitor: &V, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<V::Break> {
        match node {
            Node::SpecTlmPacketSet(spec) => spec.members.visit(a, visitor),
            _ => self.super_.visit(visitor, a, node),
        }
    }
}
