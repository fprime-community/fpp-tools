use crate::Analysis;
use crate::errors::SemanticError;
use crate::semantics::{FppSystem, Symbol, SymbolInterface};
use fpp_ast::{AstNode, DefModule, DefSystem, Visitor, Walkable};
use fpp_core::Spanned;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Check system definitions:
///   - a model may have at most one system definition;
///   - the named topology must resolve to a topology symbol;
///   - that topology must be a deployment topology.
pub struct CheckSystemDefs;

impl CheckSystemDefs {
    fn check_no_duplicate_def(a: &Analysis, symbol: &Symbol) -> Result<(), SemanticError> {
        match a.system_map.keys().next() {
            None => Ok(()),
            Some(prev_symbol) => Err(SemanticError::DuplicateSystemDefinition {
                loc: symbol.node().span(),
                prev_loc: prev_symbol.node().span(),
            }),
        }
    }
}

impl<'ast> Visitor<'ast> for CheckSystemDefs {
    type Break = ();
    type State = Analysis;

    fn visit_def_module(
        &self,
        a: &mut Self::State,
        node: &'ast DefModule,
    ) -> ControlFlow<Self::Break> {
        node.walk(a, self)
    }

    fn visit_def_system(
        &self,
        a: &mut Self::State,
        node: &'ast DefSystem,
    ) -> ControlFlow<Self::Break> {
        let symbol = Symbol::System(Arc::new(node.clone()));

        // At most one system definition per model.
        if let Err(err) = Self::check_no_duplicate_def(a, &symbol) {
            err.emit();
            return ControlFlow::Continue(());
        }

        // The named topology must resolve to a topology symbol.
        let topology_symbol = match a.get_topology_symbol(node.topology.id()) {
            Ok(Some(sym)) => sym,
            Ok(None) => return ControlFlow::Continue(()),
            Err(err) => {
                err.emit();
                return ControlFlow::Continue(());
            }
        };

        // That topology must be a deployment topology.
        if let Symbol::Topology(def) = &topology_symbol
            && !def.is_deployment
        {
            SemanticError::InvalidSymbol {
                symbol_name: def.name.data.clone(),
                msg: format!(
                    "invalid use of symbol {}: topology used here must be a deployment topology",
                    def.name.data
                ),
                loc: symbol.node().span(),
                def_loc: def.node_id.span(),
            }
            .emit();
            return ControlFlow::Continue(());
        }

        let topology = a
            .topology_map
            .get(&topology_symbol)
            .cloned()
            .expect("every topology is resolved by CheckTopologyDefs");
        a.system_map.insert(
            symbol,
            FppSystem {
                node: Arc::new(node.clone()),
                topology,
            },
        );
        ControlFlow::Continue(())
    }
}
