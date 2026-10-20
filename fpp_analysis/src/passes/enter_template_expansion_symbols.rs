use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::passes::EnterSymbols;
use crate::passes::enter_module_symbols::EnterModuleSymbols;
use crate::semantics::{Scope, ScopeOwner, Symbol, SymbolInterface, TemplateExpansion};
use fpp_ast::{
    AstNode, DefModule, DefModuleTemplate, ModuleMember, SpecTemplateExpand, TemplateArgKind,
    TemplateParam, Visitable, Visitor, Walkable,
};
use fpp_core::Spanned;
use rustc_hash::FxHashMap as HashMap;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Enter symbols introduced in template expansions
///
/// The bound parameters and the definitions of an expansion go into two scopes
/// of their own. Both are visible at the same time inside the expansion, and the
/// definitions are also merged into the enclosing scope, because they are
/// definitions of the enclosing module.
///
/// An error in binding the parameters of an expansion or in merging its
/// definitions into the enclosing scope stops the pass. The definitions
/// themselves are entered by [`EnterSymbols`], which reports a redefinition and
/// goes on past it, inside an expansion as outside one.
pub struct EnterTemplateExpansionSymbols;

impl<'ast> EnterModuleSymbols<'ast> for EnterTemplateExpansionSymbols {}

impl<'ast> Visitor<'ast> for EnterTemplateExpansionSymbols {
    type Break = ();
    type State = Analysis;

    fn visit_trans_unit(
        &self,
        a: &mut Analysis,
        node: &'ast fpp_ast::TransUnit,
    ) -> ControlFlow<Self::Break> {
        node.walk(a, self)
    }

    fn visit_def_module(&self, a: &mut Analysis, def: &'ast DefModule) -> ControlFlow<Self::Break> {
        self.def_module(a, def)
    }

    fn visit_spec_template_expand(
        &self,
        a: &mut Analysis,
        node: &'ast SpecTemplateExpand,
    ) -> ControlFlow<Self::Break> {
        let Some(members) = &node.members else {
            // ExpandTemplates leaves a specifier whose template name did not
            // resolve unexpanded, so there is nothing to enter
            return ControlFlow::Continue(());
        };
        let Ok(Some(Symbol::Template(tmpl))) = a.get_template_symbol(node.template.id()) else {
            // CheckTemplateUses reports a template name that does not resolve,
            // and the analysis goes on past that error. The members of an
            // expansion written in the source as `expand T { ... }` have no
            // parameters to bind then, so there is nothing to enter.
            return ControlFlow::Continue(());
        };

        if a.template_expansion_map.contains_key(&node.node_id) {
            // We already entered this expansion
            // Make sure we recursively enter all the symbols
            // The scope of the expansion is updated in place
            a.nested_scope
                .push(ScopeOwner::TemplateExpansion(node.node_id));
            let result = members.iter().try_for_each(|member| member.visit(a, self));
            a.nested_scope.pop();
            return result;
        }

        if let Err(err) = self.enter_expansion(a, &tmpl, node, members) {
            err.emit();
            return ControlFlow::Break(());
        }
        members.iter().try_for_each(|member| member.visit(a, self))
    }
}

impl EnterTemplateExpansionSymbols {
    /// Enter the symbols of an expansion that has not been entered yet: bind its
    /// parameters, enter its definitions, and merge the definitions into the
    /// enclosing scope
    fn enter_expansion(
        &self,
        a: &mut Analysis,
        tmpl: &Arc<DefModuleTemplate>,
        node: &SpecTemplateExpand,
        members: &[ModuleMember],
    ) -> SemanticResult {
        // Build the parameter set of the expansion
        let params = Self::bind_params(tmpl, node)?;

        // The parameter scope and the expansion scope are reached through the
        // expansion, so record it before filling either one in
        a.template_expansion_map.insert(
            node.node_id,
            TemplateExpansion {
                def_node: tmpl.clone(),
                expansion: Arc::new(node.clone()),
                params: params.clone(),
                param_scope: Scope::empty(),
                scope: Scope::empty(),
                local_scope_map: HashMap::default(),
            },
        );

        // Enter the template parameters into a new scope
        a.nested_scope
            .push(ScopeOwner::TemplateParams(node.node_id));
        let result = params.iter().try_for_each(|param| {
            a.symbol_put(TemplateExpansion::name_group(param), param.clone())
        });
        a.nested_scope.pop();
        result?;

        // Create an empty scope for entering template expanded symbols
        a.nested_scope
            .push(ScopeOwner::TemplateExpansion(node.node_id));
        // Enter the child symbols in to the analysis. EnterSymbols reports its
        // errors itself and goes on past them.
        for member in members {
            let _ = member.visit(a, &EnterSymbols);
        }
        a.nested_scope.pop();

        let template_scope = a.expansion_at(node.node_id).scope.clone();

        let mut local_scope_map = HashMap::default();
        Self::collect_module_scopes(a, &template_scope, &mut local_scope_map);
        a.template_expansion_map
            .get_mut(&node.node_id)
            .expect("expansion was just recorded")
            .local_scope_map = local_scope_map;

        // The parameter scope and the expansion scope are visible at the same
        // time, so no parameter may have the same name as a definition in the
        // expansion in the same name group
        for (param, def_param) in params.iter().zip(tmpl.params.iter()) {
            let name = &param.name().data;
            if let Some(def_symbol) = template_scope.get(TemplateExpansion::name_group(param), name)
            {
                return Err(SemanticError::TemplateParameterConflict {
                    param_name: name.clone(),
                    param_loc: def_param.span(),
                    def_loc: def_symbol.get_loc(),
                });
            }
        }

        // Duplicate the symbols entries into the outer scope
        let owner = a.nested_scope.inner_scope().clone();
        let mut dest = a.get_scope(&owner).clone();
        let result = Self::merge_scope(a, &mut dest, &template_scope);
        *a.get_scope_mut(&owner) = dest;
        result
    }

    /// Bind each argument of an expansion to the parameter it is given for,
    /// giving the parameter symbols of the expansion in the order the
    /// parameters are declared
    fn bind_params(
        tmpl: &DefModuleTemplate,
        node: &SpecTemplateExpand,
    ) -> SemanticResult<Vec<Symbol>> {
        let def_params = &tmpl.params;

        // Make sure the lengths line up
        if node.args.len() != def_params.len() {
            return Err(SemanticError::WrongNumberOfTemplateArguments {
                expand_loc: node.span(),
                def_loc: tmpl.span(),
                expand_length: node.args.len(),
                def_length: def_params.len(),
            });
        }

        // Create the parameter symbols by mapping each argument together with the definition
        // Make sure each argument is of the proper type
        node.args
            .iter()
            .zip(def_params.iter())
            .map(
                |(value_param, def_param)| match (def_param, &value_param.kind) {
                    (TemplateParam::Constant(def_param), TemplateArgKind::Constant(value)) => {
                        Ok(Symbol::TemplateConstantArg {
                            param_def: Arc::new(def_param.clone()),
                            value: Arc::new(value.clone()),
                        })
                    }
                    (TemplateParam::Type(def_param), TemplateArgKind::Type(type_name)) => {
                        Ok(Symbol::TemplateTypeArg {
                            param_def: Arc::new(def_param.clone()),
                            value: Arc::new(type_name.clone()),
                        })
                    }
                    (TemplateParam::Interface(def_param), TemplateArgKind::Interface(instance)) => {
                        Ok(Symbol::TemplateInterfaceArg {
                            param_def: Arc::new(def_param.clone()),
                            value: Arc::new(instance.clone()),
                        })
                    }
                    (dp, _) => Err(SemanticError::InvalidTemplateArg {
                        param_name: dp.name().data.clone(),
                        arg_loc: value_param.span(),
                        param_loc: dp.span(),
                        msg: format!(
                            "expected a {} argument, got a {} argument",
                            dp.kind_str(),
                            value_param.kind_str()
                        ),
                    }),
                },
            )
            .collect()
    }

    /// The scopes of the modules defined in an expansion, keyed by their symbols.
    ///
    /// These are the scopes holding just what the expansion introduced, before
    /// merging with a same-named module at the expansion site.
    fn collect_module_scopes(a: &Analysis, scope: &Scope, map: &mut HashMap<Symbol, Scope>) {
        let mut module_symbols: Vec<Symbol> = Vec::new();
        for (_, nsm) in scope.iter() {
            for (_, symbol) in nsm.iter() {
                if matches!(symbol, Symbol::Module(_)) && !module_symbols.contains(symbol) {
                    module_symbols.push(symbol.clone());
                }
            }
        }
        for symbol in module_symbols {
            if map.contains_key(&symbol) {
                continue;
            }
            if let Some(module_scope) = a.symbol_scope_map.get(&symbol).cloned() {
                map.insert(symbol, module_scope.clone());
                Self::collect_module_scopes(a, &module_scope, map);
            }
        }
    }

    /// Copy the bindings of `src` into `dest`, in the order the name groups and
    /// the names were entered into `src`
    fn merge_scope(a: &mut Analysis, dest: &mut Scope, src: &Scope) -> SemanticResult {
        for (ng, nsm) in src.iter() {
            for (_, symbol) in nsm.iter() {
                let symbol = symbol.clone();
                match dest.get(ng, &symbol.name().data) {
                    // The name is already bound to this symbol, for example
                    // because we merged it when visiting another name group
                    Some(prev_symbol) if prev_symbol == symbol => {}
                    // Two modules with the same name: merge their scopes and keep
                    // the binding that is already there
                    Some(prev_symbol @ Symbol::Module(_))
                        if matches!(symbol, Symbol::Module(_)) =>
                    {
                        Self::merge_module_scopes(a, &prev_symbol, &symbol)?;
                    }
                    _ => dest.put(ng, symbol)?,
                }
            }
        }
        Ok(())
    }

    /// Merge the scopes of two module symbols denoting the same module
    fn merge_module_scopes(
        a: &mut Analysis,
        prev_symbol: &Symbol,
        symbol: &Symbol,
    ) -> SemanticResult {
        let prev_scope = a
            .symbol_scope_map
            .get(prev_symbol)
            .cloned()
            .unwrap_or_else(Scope::empty);
        let scope = a
            .symbol_scope_map
            .get(symbol)
            .cloned()
            .unwrap_or_else(Scope::empty);
        let mut merged_scope = prev_scope;
        Self::merge_scope(a, &mut merged_scope, &scope)?;
        // Both symbols denote the same module, so both must see all its members
        a.symbol_scope_map
            .insert(prev_symbol.clone(), merged_scope.clone());
        a.symbol_scope_map.insert(symbol.clone(), merged_scope);
        Ok(())
    }
}
