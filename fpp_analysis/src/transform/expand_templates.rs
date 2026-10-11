use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::Symbol;
use fpp_ast::{
    AstNode, DefModule, DefModuleTemplate, ModuleMember, Replicator, SpecTemplateExpand, TransUnit,
};
use fpp_core::{Span, Spanned};

/// Expand any template expansion specifiers that have not yet been expanded
///
/// The specifier is replaced by the expanded template: the specifier itself,
/// carrying a copy of the syntax provided by the template definition it refers
/// to. That syntax behaves as if it were written at the point of the specifier,
/// except for the special rules for resolving names inside it, which are the
/// business of [`crate::passes::EnterTemplateExpansionSymbols`] and
/// [`crate::passes::CheckUses`].
///
/// Each copy gets its own node identities, so every expansion is a distinct set
/// of definitions. The span of each copied node still points at the template
/// body, with the specifier recorded as the point of expansion, so a diagnostic
/// reports both.
///
/// The transform walks the model in source order, and it walks the body of a
/// template both where the template is defined and where it is expanded. The
/// `template_definition` parameter of the walk is the template definition the
/// walk is inside of, if any: the rules that a template body defines no
/// template and expands no template are checked against it.
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
            Self::match_module_member(a, None, member)?;
        }
        Ok(())
    }

    fn match_module_member(
        a: &Analysis,
        template_definition: Option<Span>,
        member: &mut ModuleMember,
    ) -> SemanticResult {
        match member {
            ModuleMember::DefModule(def) => Self::def_module(a, template_definition, def),
            ModuleMember::DefModuleTemplate(def) => {
                Self::def_module_template(a, template_definition, def)
            }
            ModuleMember::SpecTemplateExpand(spec) => {
                Self::spec_template_expand(a, template_definition, spec)
            }
            _ => Ok(()),
        }
    }

    /// Check the rules that ensure expanding a template never defines another
    /// template and never causes another expansion: no template definition may
    /// be the ancestor of another template definition or of a template expansion
    /// specifier.
    fn def_module_template(
        a: &Analysis,
        template_definition: Option<Span>,
        def: &mut DefModuleTemplate,
    ) -> SemanticResult {
        // Validate no nested template
        if let Some(parent_loc) = template_definition {
            return Err(SemanticError::NestedTemplateDefinition {
                nested_loc: def.span(),
                parent_loc,
            });
        }
        let template_definition = Some(def.span());
        for member in &mut def.members {
            Self::match_module_member(a, template_definition, member)?;
        }
        Ok(())
    }

    fn def_module(
        a: &Analysis,
        template_definition: Option<Span>,
        def: &mut DefModule,
    ) -> SemanticResult {
        for member in &mut def.members {
            Self::match_module_member(a, template_definition, member)?;
        }
        Ok(())
    }

    fn spec_template_expand(
        a: &Analysis,
        template_definition: Option<Span>,
        spec: &mut SpecTemplateExpand,
    ) -> SemanticResult {
        if let Some(template_loc) = template_definition {
            // The expansion specifier is nested inside a template body
            return Err(SemanticError::TemplateExpansionInTemplate {
                expand_loc: spec.span(),
                template_loc,
            });
        }

        // Not inside a template: expand it

        // Look up the template def. CheckTemplateUses reports a template name
        // that does not resolve, and the analysis goes on past that error, so a
        // specifier whose name did not resolve is left unexpanded.
        let Some(Symbol::Template(def)) = a.get_template_symbol(spec.template.id())? else {
            return Ok(());
        };

        // Clone the template body, recording the template definition so that
        // any nested template definition or expansion specifier is rejected.
        let template_definition = Some(def.span());
        let mut members = def.members.clone();
        for member in &mut members {
            Self::match_module_member(a, template_definition, member)?;
        }

        // Give every cloned node its own identity, recording the expansion node
        // so that cloned nodes point back at the expansion site
        let replicator = Replicator::new(spec.span());
        replicator.apply(&mut members);

        // Paste the expanded template body into the expansion specifier
        spec.members = Some(members);
        Ok(())
    }
}
