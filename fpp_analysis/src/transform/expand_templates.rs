use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::Symbol;
use fpp_ast::{
    AstNode, DefModule, DefModuleTemplate, ModuleMember, Replicator, SpecTemplateExpand, TransUnit,
};
use fpp_core::Spanned;

/// Expand every template expansion specifier in a model.
///
/// The specifier is replaced by the expanded template: the specifier itself,
/// carrying a copy of the syntax provided by the template definition it refers
/// to. That syntax behaves as if it were written at the point of the specifier,
/// except for the special rules for resolving names inside it, which are the
/// business of [`crate::passes::EnterTemplateSymbols`] and
/// [`crate::passes::CheckUses`].
///
/// Each copy gets its own node identities, so every expansion is a distinct set
/// of definitions. The span of each copied node still points at the template
/// body, with the specifier recorded as the point of expansion, so a diagnostic
/// reports both.
///
/// A model that fails this transform cannot be analyzed, so the first error
/// stops the transform.
pub struct ExpandTemplates;

impl ExpandTemplates {
    /// Expand the templates in a list of translation units
    pub fn transform(a: &Analysis, tul: &mut [TransUnit]) -> SemanticResult {
        for tu in tul {
            for member in &mut tu.0 {
                Self::module_member(a, member)?;
            }
        }
        Ok(())
    }

    fn module_member(a: &Analysis, member: &mut ModuleMember) -> SemanticResult {
        match member {
            ModuleMember::DefModule(def) => Self::def_module(a, def),
            ModuleMember::DefModuleTemplate(def) => Self::def_module_template(def),
            ModuleMember::SpecTemplateExpand(spec) => Self::spec_template_expand(a, spec),
            _ => Ok(()),
        }
    }

    fn def_module(a: &Analysis, def: &mut DefModule) -> SemanticResult {
        for member in &mut def.members {
            Self::module_member(a, member)?;
        }
        Ok(())
    }

    /// Check the rules that ensure expanding a template never defines another
    /// template and never causes another expansion: no template definition may
    /// be the ancestor of another template definition or of a template expansion
    /// specifier.
    fn def_module_template(def: &mut DefModuleTemplate) -> SemanticResult {
        let template_loc = def.span();
        for member in &mut def.members {
            Self::template_body_member(member, template_loc)?;
        }
        Ok(())
    }

    /// A member of a template body, or of a module nested in one
    fn template_body_member(
        member: &mut ModuleMember,
        template_loc: fpp_core::Span,
    ) -> SemanticResult {
        match member {
            ModuleMember::DefModule(def) => {
                for member in &mut def.members {
                    Self::template_body_member(member, template_loc)?;
                }
                Ok(())
            }
            ModuleMember::DefModuleTemplate(def) => Err(SemanticError::NestedTemplateDefinition {
                nested_loc: def.span(),
                parent_loc: template_loc,
            }),
            ModuleMember::SpecTemplateExpand(spec) => {
                Err(SemanticError::TemplateExpansionInTemplate {
                    expand_loc: spec.span(),
                    template_loc,
                })
            }
            _ => Ok(()),
        }
    }

    fn spec_template_expand(a: &Analysis, spec: &mut SpecTemplateExpand) -> SemanticResult {
        // Look up the template definition. The name is resolved by
        // CheckTemplateUses, which has already reported an unresolved one, so
        // there is nothing left to expand for it.
        let Some(Symbol::Template(def)) = a.get_template_symbol(spec.template.id())? else {
            return Ok(());
        };

        // Copy the template body to the point of the specifier, giving every
        // copied node its own identity
        let replicator = Replicator::new(spec.span());
        let mut members = def.members.clone();
        replicator.apply(&mut members);
        spec.members = Some(members);
        Ok(())
    }
}
