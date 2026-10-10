use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::passes::EnterSymbols;
use crate::semantics::{NameGroup, Scope, ScopeOwner, Symbol, SymbolInterface, TemplateExpansion};
use fpp_ast::{
    AstNode, DefModule, SpecTemplateExpand, TemplateArgKind, TemplateParam, Visitable, Visitor,
    Walkable,
};
use fpp_core::Spanned;
use rustc_hash::FxHashMap as HashMap;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Enter the symbols introduced by each template expansion, and bind each
/// template argument to the parameter it is given for.
///
/// The bound parameters and the definitions of the expansion go into two scopes
/// of their own. Both are visible at the same time inside the expansion, and the
/// definitions are also merged into the enclosing scope, because they are
/// definitions of the enclosing module.
///
/// A model that fails this pass cannot be analyzed, so the first error stops the
/// pass.
pub struct EnterTemplateSymbols;

impl EnterTemplateSymbols {
    /// Bind each argument of an expansion to the parameter it is given for
    fn bind_params(
        &self,
        def: &Arc<fpp_ast::DefModuleTemplate>,
        node: &SpecTemplateExpand,
    ) -> SemanticResult<Vec<Symbol>> {
        if node.args.len() != def.params.len() {
            return Err(SemanticError::WrongNumberOfTemplateArguments {
                expand_loc: node.span(),
                def_loc: def.span(),
                expand_length: node.args.len(),
                def_length: def.params.len(),
            });
        }

        let mut params = Vec::with_capacity(def.params.len());
        for (arg, param) in node.args.iter().zip(def.params.iter()) {
            let symbol = match (param, &arg.kind) {
                (TemplateParam::Constant(p), TemplateArgKind::Constant(e)) => {
                    Symbol::TemplateConstantArg {
                        param_def: Arc::new(p.clone()),
                        value: Arc::new(e.clone()),
                    }
                }
                (TemplateParam::Type(p), TemplateArgKind::Type(tn)) => Symbol::TemplateTypeArg {
                    param_def: Arc::new(p.clone()),
                    value: Arc::new(tn.clone()),
                },
                (TemplateParam::Interface(p), TemplateArgKind::Interface(q)) => {
                    Symbol::TemplateInterfaceArg {
                        param_def: Arc::new(p.clone()),
                        value: Arc::new(q.clone()),
                    }
                }
                _ => {
                    return Err(SemanticError::InvalidTemplateArg {
                        param_name: param.name().data.clone(),
                        arg_loc: arg.span(),
                        param_loc: param.span(),
                        msg: format!(
                            "expected a {} argument, got a {} argument",
                            param.kind_str(),
                            arg.kind_str()
                        ),
                    });
                }
            };
            params.push(symbol);
        }
        Ok(params)
    }

    /// The scopes of the modules defined in an expansion, keyed by their symbols.
    ///
    /// These are the scopes holding just what the expansion introduced, before
    /// merging with a same-named module at the expansion site.
    fn collect_module_scopes(a: &Analysis, scope: &Scope, map: &mut HashMap<Symbol, Scope>) {
        let mut modules: Vec<Symbol> = Vec::new();
        for ng in NameGroup::all() {
            for (_, symbol) in scope.get_group(ng).iter() {
                if matches!(symbol, Symbol::Module(_)) && !modules.contains(symbol) {
                    modules.push(symbol.clone());
                }
            }
        }
        for symbol in modules {
            if map.contains_key(&symbol) {
                continue;
            }
            if let Some(module_scope) = a.symbol_scope_map.get(&symbol).cloned() {
                map.insert(symbol, module_scope.clone());
                Self::collect_module_scopes(a, &module_scope, map);
            }
        }
    }

    /// Copy the bindings of `src` into `dest`
    fn merge_scope(a: &mut Analysis, dest: &mut Scope, src: &Scope) -> SemanticResult {
        for ng in NameGroup::all() {
            let entries: Vec<Symbol> = src
                .get_group(ng)
                .iter()
                .map(|(_, symbol)| symbol.clone())
                .collect();
            for symbol in entries {
                match dest.get(ng, &symbol.name().data) {
                    // The name is already bound to this symbol, for example
                    // because we merged it when visiting another name group
                    Some(prev) if prev == symbol => {}
                    // Two modules with the same name: merge their scopes and keep
                    // the binding that is already there
                    Some(prev @ Symbol::Module(_)) if matches!(symbol, Symbol::Module(_)) => {
                        Self::merge_module_scopes(a, &prev, &symbol)?;
                    }
                    _ => dest.put(ng, symbol)?,
                }
            }
        }
        Ok(())
    }

    /// Merge the scopes of two module symbols denoting the same module
    fn merge_module_scopes(a: &mut Analysis, prev: &Symbol, symbol: &Symbol) -> SemanticResult {
        let mut merged = a
            .symbol_scope_map
            .get(prev)
            .cloned()
            .unwrap_or_else(Scope::new);
        let scope = a
            .symbol_scope_map
            .get(symbol)
            .cloned()
            .unwrap_or_else(Scope::new);
        Self::merge_scope(a, &mut merged, &scope)?;
        // Both symbols denote the same module, so both must see all its members
        a.symbol_scope_map.insert(prev.clone(), merged.clone());
        a.symbol_scope_map.insert(symbol.clone(), merged);
        Ok(())
    }

    fn enter_expansion(
        &self,
        a: &mut Analysis,
        node: &SpecTemplateExpand,
        members: &[fpp_ast::ModuleMember],
    ) -> SemanticResult {
        let Some(Symbol::Template(def)) = a.get_template_symbol(node.template.id())? else {
            // ExpandTemplates copies a body only for a resolved template name, so
            // there are no members to enter without one
            return Ok(());
        };

        let params = self.bind_params(&def, node)?;

        // The parameter scope and the expansion scope are reached through the
        // expansion, so record it before filling either one in
        a.template_expansion_map.insert(
            node.node_id,
            TemplateExpansion {
                def_node: def.clone(),
                expansion: Arc::new(node.clone()),
                params: params.clone(),
                param_scope: Scope::new(),
                scope: Scope::new(),
                local_scope_map: HashMap::default(),
            },
        );

        // Enter the template parameters into a scope of their own
        a.nested_scope
            .push(ScopeOwner::TemplateParams(node.node_id));
        let result = (|| -> SemanticResult {
            for param in &params {
                a.symbol_put(TemplateExpansion::name_group(param), param.clone())?;
            }
            Ok(())
        })();
        a.nested_scope.pop();
        result?;

        // Enter the definitions of the expansion into a scope of their own
        a.nested_scope
            .push(ScopeOwner::TemplateExpansion(node.node_id));
        for member in members {
            let _ = member.visit(a, &EnterSymbols);
        }
        a.nested_scope.pop();

        let expansion_scope = a.expansion_at(node.node_id).scope.clone();

        let mut local_scope_map = HashMap::default();
        Self::collect_module_scopes(a, &expansion_scope, &mut local_scope_map);
        a.template_expansion_map
            .get_mut(&node.node_id)
            .expect("expansion was just recorded")
            .local_scope_map = local_scope_map;

        // The parameter scope and the expansion scope are visible at the same
        // time, so no parameter may have the same name as a definition in the
        // expansion in the same name group
        for (param, def_param) in params.iter().zip(def.params.iter()) {
            let name = &param.name().data;
            if let Some(def_symbol) =
                expansion_scope.get(TemplateExpansion::name_group(param), name)
            {
                return Err(SemanticError::TemplateParameterConflict {
                    param_name: name.clone(),
                    param_loc: def_param.span(),
                    def_loc: def_symbol.node().span(),
                });
            }
        }

        // The definitions of the expansion are definitions of the enclosing
        // module, so they are visible outside the expansion too
        let owner = a.nested_scope.current().clone();
        let mut dest = a.get_scope(&owner).clone();
        let result = Self::merge_scope(a, &mut dest, &expansion_scope);
        *a.get_scope_mut(&owner) = dest;
        result
    }
}

impl<'ast> Visitor<'ast> for EnterTemplateSymbols {
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
        let Some(symbol) = a.get_symbol(def) else {
            return ControlFlow::Continue(());
        };
        a.nested_scope.push(ScopeOwner::Symbol(symbol.clone()));
        let saved_parent = a.parent_symbol.replace(symbol);
        let res = def.walk(a, self);
        a.parent_symbol = saved_parent;
        a.nested_scope.pop();
        res
    }

    fn visit_spec_template_expand(
        &self,
        a: &mut Analysis,
        node: &'ast SpecTemplateExpand,
    ) -> ControlFlow<Self::Break> {
        let Some(members) = &node.members else {
            // This specifier was not expanded, so there is nothing to enter
            return ControlFlow::Continue(());
        };
        if a.template_expansion_map.contains_key(&node.node_id) {
            // We already entered this expansion
            return ControlFlow::Continue(());
        }
        match self.enter_expansion(a, node, members) {
            Ok(()) => ControlFlow::Continue(()),
            Err(err) => {
                err.emit();
                ControlFlow::Break(())
            }
        }
    }
}
