use crate::Analysis;
use crate::errors::SemanticError;
use fpp_ast::{
    DefModule, DefModuleTemplate, SpecTemplateExpand, TransUnit, Visitable, Visitor, Walkable,
};
use fpp_core::Spanned;
use std::ops::ControlFlow;

/// Check template expansions
///
/// The members of an expanded template, whether written in the source or produced
/// by template expansion, may contain no module template definition and no
/// template expansion specifier. The rule ensures that the members written in the
/// source could have been produced by expanding some template.
///
/// The first error stops the pass: template expansion relies on these rules.
pub struct CheckTemplateExpansions;

impl CheckTemplateExpansions {
    fn spec_template_expand_unexpanded(
        &self,
        a: &mut Analysis,
        node: &SpecTemplateExpand,
    ) -> ControlFlow<()> {
        match a.template_expansion_id {
            Some(expand_id) => {
                SemanticError::TemplateExpandError {
                    loc: node.span(),
                    msg: "template expansions cannot be nested".to_string(),
                    expand_loc: expand_id.span(),
                }
                .emit();
                ControlFlow::Break(())
            }
            None => ControlFlow::Continue(()),
        }
    }
}

impl<'ast> Visitor<'ast> for CheckTemplateExpansions {
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
        match a.template_expansion_id {
            Some(expand_id) => {
                SemanticError::TemplateExpandError {
                    loc: def.span(),
                    msg: "module template definitions cannot appear inside a template expansion"
                        .to_string(),
                    expand_loc: expand_id.span(),
                }
                .emit();
                ControlFlow::Break(())
            }
            None => ControlFlow::Continue(()),
        }
    }

    fn visit_spec_template_expand(
        &self,
        a: &mut Analysis,
        node: &'ast SpecTemplateExpand,
    ) -> ControlFlow<()> {
        self.spec_template_expand_unexpanded(a, node)?;
        let Some(members) = &node.members else {
            return ControlFlow::Continue(());
        };
        a.template_expansion_id = Some(node.node_id);
        let result = members.iter().try_for_each(|member| member.visit(a, self));
        a.template_expansion_id = None;
        result
    }
}
