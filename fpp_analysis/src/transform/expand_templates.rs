use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::Symbol;
use fpp_ast::{AstNode, ModuleMember, Replicator, SpecTemplateExpand, TransUnit};
use fpp_core::Spanned;

/// Expand any template expansion specifiers that have not yet been expanded
///
/// An unexpanded specifier is replaced by the expanded template: the specifier
/// itself, carrying a copy of the syntax provided by the template definition it
/// refers to. That syntax behaves as if it were written at the point of the
/// specifier, except for the special rules for resolving names inside it, which
/// are the business of [`crate::passes::EnterTemplateExpansionSymbols`] and
/// [`crate::passes::CheckUses`]. An expanded specifier, whether written in the
/// source as `expand T { ... }` or produced by an earlier expansion, is left
/// unchanged, so expanding a model twice is the same as expanding it once.
///
/// Each copy gets its own node identities, so every expansion is a distinct set
/// of definitions. The span of each copied node still points at the template
/// body, with the specifier recorded as the point of expansion, so a diagnostic
/// reports both.
///
/// [`crate::passes::CheckTemplateDefs`] has checked that no template body
/// defines a template or expands one, so a copy needs no further expansion, and
/// the transform does not visit template bodies.
///
/// A model that fails this transform cannot be analyzed, so the first error
/// stops the transform.
pub struct ExpandTemplates;

impl ExpandTemplates {
    /// Transform a list in sequence
    pub fn transform_list(a: &Analysis, tul: &mut [TransUnit]) -> SemanticResult {
        for tu in tul {
            Self::trans_unit(a, tu)?;
        }
        Ok(())
    }

    pub fn trans_unit(a: &Analysis, tu: &mut TransUnit) -> SemanticResult {
        for member in &mut tu.0 {
            Self::match_module_member(a, member)?;
        }
        Ok(())
    }

    fn match_module_member(a: &Analysis, member: &mut ModuleMember) -> SemanticResult {
        match member {
            ModuleMember::DefModule(def) => {
                for member in &mut def.members {
                    Self::match_module_member(a, member)?;
                }
                Ok(())
            }
            ModuleMember::SpecTemplateExpand(spec) => Self::spec_template_expand(a, spec),
            _ => Ok(()),
        }
    }

    fn spec_template_expand(a: &Analysis, spec: &mut SpecTemplateExpand) -> SemanticResult {
        match spec.members {
            // Members are already there; nothing to do
            Some(_) => Ok(()),
            // Members are not there; expand
            None => Self::expand_template(a, spec),
        }
    }

    /// Expand a template
    fn expand_template(a: &Analysis, spec: &mut SpecTemplateExpand) -> SemanticResult {
        // Look up the template def. CheckTemplateUses reports a template name
        // that does not resolve, and the analysis goes on past that error, so a
        // specifier whose name did not resolve is left unexpanded.
        let Some(Symbol::Template(def)) = a.get_template_symbol(spec.template.id())? else {
            return Ok(());
        };

        // Clone the template body, giving every cloned node its own identity and
        // recording the expansion node so that cloned nodes point back at the
        // expansion site
        let mut members = def.members.clone();
        let replicator = Replicator::new(spec.span());
        replicator.apply(&mut members);

        // Paste the expanded template body into the expansion specifier
        spec.members = Some(members);
        Ok(())
    }
}
