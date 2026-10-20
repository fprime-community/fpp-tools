use crate::Analysis;
use crate::errors::SemanticError;
use fpp_ast::{
    DefModule, DefModuleTemplate, SpecTemplateExpand, TransUnit, Visitable, Visitor, Walkable,
};
use fpp_core::Spanned;
use std::ops::ControlFlow;

/// Check template definitions
///
/// No module template definition may contain another module template definition
/// or a template expansion specifier, so expanding a template never defines
/// another template and never causes another expansion.
///
/// The first error stops the pass: template expansion relies on these rules.
pub struct CheckTemplateDefs;

impl<'ast> Visitor<'ast> for CheckTemplateDefs {
    type Break = ();
    type State = Analysis;

    fn visit_trans_unit(&self, a: &mut Analysis, node: &'ast TransUnit) -> ControlFlow<()> {
        node.walk(a, self)
    }

    fn visit_def_module(&self, a: &mut Analysis, def: &'ast DefModule) -> ControlFlow<()> {
        def.walk(a, self)
    }

    fn visit_def_module_template(
        &self,
        a: &mut Analysis,
        def: &'ast DefModuleTemplate,
    ) -> ControlFlow<()> {
        if let Some(def_id) = a.template_def_id {
            SemanticError::TemplateDefError {
                loc: def.span(),
                msg: "module template definitions cannot be nested".to_string(),
                def_loc: def_id.span(),
            }
            .emit();
            return ControlFlow::Break(());
        }
        a.template_def_id = Some(def.node_id);
        let result = def
            .members
            .iter()
            .try_for_each(|member| member.visit(a, self));
        a.template_def_id = None;
        result
    }

    fn visit_spec_template_expand(
        &self,
        a: &mut Analysis,
        node: &'ast SpecTemplateExpand,
    ) -> ControlFlow<()> {
        match a.template_def_id {
            Some(def_id) => {
                SemanticError::TemplateDefError {
                    loc: node.span(),
                    msg: "template expansion specifiers cannot appear inside a module template"
                        .to_string(),
                    def_loc: def_id.span(),
                }
                .emit();
                ControlFlow::Break(())
            }
            None => ControlFlow::Continue(()),
        }
    }
}
