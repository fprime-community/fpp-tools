use crate::Analysis;
use crate::analyzers::module_analyzer::visit_expansion;
use crate::passes::check_uses::check_qual_ident;
use crate::semantics::{NameGroup, ScopeOwner, Symbol};
use fpp_ast::{DefModule, SpecTemplateExpand, TransUnit, Visitor, Walkable};
use std::ops::ControlFlow;

/// Match the name in each template expansion specifier to the template
/// definition it refers to.
///
/// This runs before templates are expanded, so that expansion knows which
/// template body to copy. Nothing else in an expansion specifier can be
/// analyzed yet: the arguments bind to parameters only once the specifier is
/// matched with its definition.
///
/// An expansion specifier is a module member, so the pass visits only
/// translation unit members and module members.
pub struct CheckTemplateUses;

impl<'ast> Visitor<'ast> for CheckTemplateUses {
    type Break = ();
    type State = Analysis;

    fn visit_trans_unit(&self, a: &mut Analysis, node: &'ast TransUnit) -> ControlFlow<()> {
        node.walk(a, self)
    }

    fn visit_def_module(&self, a: &mut Analysis, def: &'ast DefModule) -> ControlFlow<()> {
        let symbol = match a.symbol_get(NameGroup::Value, &def.name.data) {
            Some(symbol @ Symbol::Module(_)) => symbol,
            // The analysis goes on past a redefinition, so the name of a module
            // can be bound to another definition. EnterSymbols has reported that
            // and entered a module symbol of its own for the definition.
            _ => a
                .get_symbol(def)
                .expect("EnterSymbols enters a symbol for every module definition"),
        };
        a.nested_scope.push(ScopeOwner::Symbol(symbol));
        let result = def.walk(a, self);
        a.nested_scope.pop();
        result
    }

    fn visit_spec_template_expand(
        &self,
        a: &mut Analysis,
        node: &'ast SpecTemplateExpand,
    ) -> ControlFlow<()> {
        visit_expansion(self, a, node)?;
        check_qual_ident(a, NameGroup::Template, &node.template);
        ControlFlow::Continue(())
    }
}
