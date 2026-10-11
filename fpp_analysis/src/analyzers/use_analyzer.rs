use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{BasicUseAnalyzer, UseAnalysisPass};
use crate::semantics::QualifiedName;
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
                            // e is a use, so it must be a constant use. A use
                            // that names some other kind of symbol is still a
                            // use: the constant use of a pass reports it.
                            Some(_) => {
                                let use_name = QualifiedName::from(
                                    expr.get_ident_list_opt()
                                        .expect("expected a qualified identifier"),
                                );
                                visitor.constant_use(a, expr, use_name)
                            }
                            None => {
                                // e is not a use, so it selects a member of a struct value
                                // Analyze the left-hand expression representing the struct value
                                e.visit(a, visitor)
                            }
                        }
                    }
                    _ => self.super_.visit(visitor, a, node),
                }
            }
            _ => self.super_.visit(visitor, a, node),
        }
    }
}
