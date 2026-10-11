use crate::Analysis;
use crate::semantics::{
    Command, ComponentInstance, Dictionary, ImpliedUseKind, Symbol, SymbolInterface, Topology,
};
use crate::used_symbols::UsedSymbols;
use fpp_ast::Visitable;
use rustc_hash::FxHashSet as HashSet;
use std::sync::Arc;

/// Fills in the used symbols for a dictionary
pub struct DictionaryUsedSymbols<'a> {
    t: &'a Topology,
}

impl<'a> DictionaryUsedSymbols<'a> {
    pub fn new(t: &'a Topology) -> DictionaryUsedSymbols<'a> {
        DictionaryUsedSymbols { t }
    }

    pub fn update_used_symbols(&self, a: &mut Analysis, d: &mut Dictionary) {
        d.used_symbol_set = self.get_used_symbol_set(a);
    }

    fn get_used_symbol_set(&self, a: &mut Analysis) -> HashSet<Symbol> {
        let mut ss: HashSet<Symbol> = HashSet::default();
        let topology_node = self.t.symbol.node();
        for kind in [ImpliedUseKind::Type, ImpliedUseKind::Constant] {
            for iu in a.get_implied_uses(kind, topology_node) {
                if let Some(symbol) = a.use_def_map.get(&iu.id()).cloned() {
                    ss.insert(symbol);
                }
            }
        }
        for ci in self.t.component_instance_map().into_keys() {
            ss.extend(self.get_used_symbols_for_instance(a, &ci));
        }
        ss.extend(a.dictionary_symbol_set.iter().cloned());
        UsedSymbols::new().resolve_uses(a, &ss)
    }

    fn get_used_symbols_for_instance(
        &self,
        a: &mut Analysis,
        ci: &ComponentInstance,
    ) -> HashSet<Symbol> {
        let component = ci.component(a);
        let commands: Vec<_> = component
            .command_map
            .values()
            .filter_map(|command| match command {
                Command::NonParam { node, .. } => Some(node.clone()),
                // A parameter command is implied by a parameter specifier, which
                // is visited through the parameter map, so it adds nothing here
                _ => None,
            })
            .collect();
        let events: Vec<_> = component
            .event_map
            .values()
            .map(|e| e.node.clone())
            .collect();
        let tlm_channels: Vec<_> = component
            .tlm_channel_map
            .values()
            .map(|c| c.node.clone())
            .collect();
        let params: Vec<_> = component
            .param_map
            .values()
            .map(|p| p.node.clone())
            .collect();
        let records: Vec<_> = component
            .record_map
            .values()
            .map(|r| r.node.clone())
            .collect();
        let containers: Vec<_> = component
            .container_map
            .values()
            .map(|c| c.node.clone())
            .collect();

        let used_symbols = UsedSymbols::new();
        let command_symbols = get_used_symbols_for_specifier(a, &used_symbols, &commands);
        let event_symbols = get_used_symbols_for_specifier(a, &used_symbols, &events);
        let tlm_channel_symbols = get_used_symbols_for_specifier(a, &used_symbols, &tlm_channels);
        let param_symbols = get_used_symbols_for_specifier(a, &used_symbols, &params);
        let record_symbols = get_used_symbols_for_specifier(a, &used_symbols, &records);
        let container_symbols = get_used_symbols_for_specifier(a, &used_symbols, &containers);
        [
            command_symbols,
            event_symbols,
            tlm_channel_symbols,
            param_symbols,
            record_symbols,
            container_symbols,
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

/// The symbols used by a list of specifiers
fn get_used_symbols_for_specifier<'n, N: Visitable<'n, UsedSymbols<'n>>>(
    a: &mut Analysis,
    used_symbols: &UsedSymbols<'n>,
    specifiers: &'n [Arc<N>],
) -> HashSet<Symbol> {
    let mut out = HashSet::default();
    for specifier in specifiers {
        let saved = std::mem::take(&mut a.used_symbol_set);
        let _ = specifier.as_ref().visit(a, used_symbols);
        out.extend(std::mem::replace(&mut a.used_symbol_set, saved));
    }
    out
}
