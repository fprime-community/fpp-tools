use crate::Analysis;
use crate::analyzers::analyzer::{Analyzer, DefaultAnalyzer};
use fpp_ast::{Node, Visitable, Visitor};
use std::ops::ControlFlow;

/// Analyze enum constants
#[derive(Default)]
pub struct EnumAnalyzer<A = DefaultAnalyzer> {
    super_: A,
}

impl<'ast, V, A> Analyzer<'ast, V> for EnumAnalyzer<A>
where
    V: Visitor<'ast, State = Analysis>,
    A: Analyzer<'ast, V>,
{
    fn visit(&self, visitor: &V, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<V::Break> {
        match node {
            Node::DefEnum(def) => {
                a.scope_name_list.insert(0, def.name.data.clone());
                let result = def.constants.visit(a, visitor);
                a.scope_name_list.remove(0);
                result
            }
            _ => self.super_.visit(visitor, a, node),
        }
    }
}
