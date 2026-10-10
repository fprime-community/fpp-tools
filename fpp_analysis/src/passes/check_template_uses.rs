use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{BasicUseAnalyzer, UseAnalysisPass};
use crate::passes::check_uses::check_qual_ident;
use crate::semantics::{NameGroup, QualifiedName};
use fpp_ast::{Node, QualIdent, Visitor};
use std::ops::ControlFlow;

/// Match the name in each template expansion specifier to the template
/// definition it refers to.
///
/// This runs before templates are expanded, so that expansion knows which
/// template body to copy. Nothing else in an expansion specifier can be
/// analyzed yet: the arguments bind to parameters only once the specifier is
/// matched with its definition.
pub struct CheckTemplateUses<'ast> {
    super_: BasicUseAnalyzer<'ast, Analysis, Self>,
}

impl<'ast> Default for CheckTemplateUses<'ast> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> CheckTemplateUses<'ast> {
    pub fn new() -> CheckTemplateUses<'ast> {
        Self {
            super_: BasicUseAnalyzer::new(),
        }
    }
}

impl<'ast> Visitor<'ast> for CheckTemplateUses<'ast> {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    /// The parameter list of a template definition is checked by `CheckUses`,
    /// once the whole model is in scope.
    fn visit_def_module_template(
        &self,
        _a: &mut Analysis,
        _def: &'ast fpp_ast::DefModuleTemplate,
    ) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }
}

impl<'ast> UseAnalysisPass<'ast, Analysis> for CheckTemplateUses<'ast> {
    fn template_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = name;
        check_qual_ident(a, NameGroup::Template, node);
        ControlFlow::Continue(())
    }
}
