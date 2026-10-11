use crate::Analysis;
use crate::analyzers::analyzer::{Analyzer, DefaultAnalyzer};
use fpp_ast::{Node, SpecTemplateExpand, TemplateArgKind, Visitable, Visitor};
use std::ops::ControlFlow;

/// Analyze template expansions members
///
/// The analyzer analyzes the parameters on a template expansion specifier, and
/// then the specifier as the analyzer it wraps does. The parameters are the
/// expression or type name bound to each constant or type parameter. A bound
/// parameter holds a copy of its argument with the argument's node identity, so
/// visiting the arguments of the specifier visits the bound values, whether or
/// not the expansion exists yet. A port interface instance argument names no
/// type or expression.
///
/// A use analysis analyzes the parameters through the template hooks of
/// [`crate::analyzers::UseAnalysisPass`] instead.
#[derive(Default)]
pub struct TemplateExpandAnalyzer<A = DefaultAnalyzer> {
    super_: A,
}

impl<A> TemplateExpandAnalyzer<A> {
    /// Analyze the parameters on a template expansion
    fn template_params<'ast, V>(
        visitor: &V,
        a: &mut Analysis,
        node: &'ast SpecTemplateExpand,
    ) -> ControlFlow<V::Break>
    where
        V: Visitor<'ast, State = Analysis>,
    {
        for arg in &node.args {
            match &arg.kind {
                TemplateArgKind::Constant(e) => e.visit(a, visitor)?,
                TemplateArgKind::Type(tn) => tn.visit(a, visitor)?,
                TemplateArgKind::Interface(_) => {}
            }
        }
        ControlFlow::Continue(())
    }
}

impl<'ast, V, A> Analyzer<'ast, V> for TemplateExpandAnalyzer<A>
where
    V: Visitor<'ast, State = Analysis>,
    A: Analyzer<'ast, V>,
{
    fn visit(&self, visitor: &V, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<V::Break> {
        match node {
            Node::SpecTemplateExpand(def) => {
                // Analyze the parameters on this template expansion.
                Self::template_params(visitor, a, def)?;
                self.super_.visit(visitor, a, node)
            }
            _ => self.super_.visit(visitor, a, node),
        }
    }
}
