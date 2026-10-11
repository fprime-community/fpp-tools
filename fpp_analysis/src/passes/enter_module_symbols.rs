use crate::analysis::Analysis;
use crate::errors::SemanticError;
use crate::semantics::{NameGroup, Scope, ScopeOwner, Symbol, SymbolInterface};
use fpp_ast::{DefModule, Visitor, Walkable};
use fpp_core::Spanned;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Enter module symbols into their scopes
///
/// This is the module handling shared by [`crate::passes::EnterSymbols`] and
/// [`crate::passes::EnterTemplateExpansionSymbols`]: each implements
/// `visit_def_module` with [`EnterModuleSymbols::def_module`].
pub(crate) trait EnterModuleSymbols<'ast>:
    Visitor<'ast, State = Analysis, Break = ()>
{
    fn def_module(&self, a: &mut Analysis, def: &'ast DefModule) -> ControlFlow<()> {
        let name = &def.name.data;
        let old_scope_name_list = a.scope_name_list.clone();
        a.scope_name_list.insert(0, name.clone());
        let parent_symbol = a.parent_symbol.clone();

        let existing_symbol = a
            .get_scope(a.nested_scope.current())
            .get(NameGroup::Value, name);
        let symbol = match existing_symbol {
            Some(symbol @ Symbol::Module(_)) => {
                // We found a module symbol with the same name at the current level.
                // Re-open the scope.
                symbol
            }
            Some(symbol) => match a.symbol_map.get(&def.node_id) {
                // The analysis goes on past a redefinition, so a later pass can
                // meet this module again: the pass that reported the redefinition
                // entered a module symbol of its own for it, and that is the
                // symbol to re-open
                Some(module @ Symbol::Module(_)) => module.clone(),
                _ => {
                    // We found a non-module symbol with the same name at the
                    // current level. This is an error.
                    SemanticError::RedefinedSymbol {
                        name: name.clone(),
                        loc: def.span(),
                        prev_loc: symbol.get_loc(),
                    }
                    .emit();
                    // Go on with a module symbol that is in no scope, so that the
                    // members of the module are still entered
                    let module = Symbol::Module(Arc::new(def.into()));
                    a.symbol_scope_map.insert(module.clone(), Scope::new());
                    module
                }
            },
            None => {
                // We did not find a symbol with the same name at the current level.
                // Create a new module symbol now.
                //
                // A module definition resides in all the name groups, the system
                // name group included (FPP spec, Scoping of Names).
                let module = Symbol::Module(Arc::new(def.into()));
                a.symbol_scope_map.insert(module.clone(), Scope::new());
                for ng in NameGroup::all() {
                    if let Err(err) = a.symbol_put(ng, module.clone()) {
                        err.emit();
                    }
                }
                module
            }
        };
        // The symbol map records the module symbol that EnterSymbols opened or
        // re-opened for the definition. A later pass may re-open the merged
        // module of an expansion under the same name, but the definition still
        // denotes the module symbol of the expansion.
        a.symbol_map
            .entry(def.node_id)
            .or_insert_with(|| symbol.clone());

        a.nested_scope.push(ScopeOwner::Symbol(symbol.clone()));
        a.parent_symbol = Some(symbol.clone());
        let result = def.walk(a, self);
        a.nested_scope.pop();
        a.parent_symbol = parent_symbol;
        a.scope_name_list = old_scope_name_list;
        self.update_map(a, symbol);

        result
    }

    /// Record the current parent symbol as the parent of `s`
    fn update_map(&self, a: &mut Analysis, s: Symbol) {
        if let Some(parent) = &a.parent_symbol {
            a.parent_symbol_map.insert(s, parent.clone());
        }
    }
}
