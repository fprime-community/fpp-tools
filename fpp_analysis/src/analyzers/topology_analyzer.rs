use crate::Analysis;
use crate::analyzers::analyzer::{Analyzer, DefaultAnalyzer};
use fpp_ast::{Node, Visitable, Visitor};
use std::ops::ControlFlow;

/// Analyze topology members
#[derive(Default)]
pub struct TopologyAnalyzer<A = DefaultAnalyzer> {
    super_: A,
}

impl<'ast, V, A> Analyzer<'ast, V> for TopologyAnalyzer<A>
where
    V: Visitor<'ast, State = Analysis>,
    A: Analyzer<'ast, V>,
{
    fn visit(&self, visitor: &V, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<V::Break> {
        match node {
            Node::DefTopology(def) => {
                a.scope_name_list.insert(0, def.name.data.clone());
                let result = def.members.visit(a, visitor);
                a.scope_name_list.remove(0);
                result
            }
            _ => self.super_.visit(visitor, a, node),
        }
    }
}
