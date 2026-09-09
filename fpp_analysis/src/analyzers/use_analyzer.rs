use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{BasicUseAnalyzer, UseAnalysisPass};
use crate::semantics::Symbol;
use fpp_ast::{AstNode, ExprKind, Node, Visitable};
use std::ops::ControlFlow;

/// Analyze uses
/// This analyzer assumes that CheckUses has already been run to populate the use-def map
pub struct UseAnalyzer<'ast, V: UseAnalysisPass<'ast, Analysis>> {
    super_: BasicUseAnalyzer<'ast, Analysis, V>,
}

impl<'ast, V: UseAnalysisPass<'ast, Analysis>> Default for UseAnalyzer<'ast, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast, V: UseAnalysisPass<'ast, Analysis>> UseAnalyzer<'ast, V> {
    pub fn new() -> UseAnalyzer<'ast, V> {
        UseAnalyzer {
            super_: BasicUseAnalyzer::new(),
        }
    }
}

impl<'ast, V: UseAnalysisPass<'ast, Analysis>> Analyzer<'ast, V> for UseAnalyzer<'ast, V> {
    fn visit(&self, visitor: &V, a: &mut V::State, node: Node<'ast>) -> ControlFlow<V::Break> {
        match node {
            Node::Expr(expr) => {
                match &expr.kind {
                    ExprKind::Dot { e, .. } => {
                        match a.use_def_map.get(&expr.id()) {
                            // expr is a use, so it must be a constant use
                            Some(Symbol::Constant(_) | Symbol::EnumConstant(_)) => {
                                let use_name = self
                                    .super_
                                    .expr_to_qualified_name(expr)
                                    .expect("expected a qualified name");
                                visitor.constant_use(a, expr, use_name)
                            }
                            Some(Symbol::Module(_) | Symbol::Component(_)) | None => {
                                // expr is not a use, so it selects a member of a struct value
                                // Analyze the left-hand expression representing the struct value
                                e.visit(a, visitor)
                            }
                            // Some other kind of symbol, which should not occur here
                            Some(_) => ControlFlow::Continue(()),
                        }
                    }
                    _ => self.super_.visit(visitor, a, node),
                }
            }
            _ => self.super_.visit(visitor, a, node),
        }
    }
}
