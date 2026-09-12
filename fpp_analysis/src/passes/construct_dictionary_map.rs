use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::{
    Dictionary, DictionaryEntries, DictionaryUsedSymbols, Symbol, SymbolInterface, TlmPacket,
    TlmPacketSet, Topology, cmp_span,
};
use fpp_ast::{SpecTlmPacketSet, TlmPacketSetMember, TopologyMember};
use std::sync::Arc;

/// Construct the dictionary map
pub struct ConstructDictionaryMap;

impl ConstructDictionaryMap {
    pub fn construct(a: &mut Analysis) {
        // Visit the topologies in source order, for stable diagnostics.
        let mut symbols: Vec<Symbol> = a
            .topology_map
            .keys()
            .filter(|s| matches!(s, Symbol::Topology(def) if def.is_deployment))
            .cloned()
            .collect();
        symbols.sort_by(|x, y| cmp_span(&x.get_loc(), &y.get_loc()));
        for symbol in symbols {
            let Symbol::Topology(def) = symbol.clone() else {
                continue;
            };
            let Some(t) = a.topology_map.get(&symbol).cloned() else {
                continue;
            };
            let mut d = Dictionary::default();
            DictionaryUsedSymbols::new(&t).update_used_symbols(a, &mut d);
            DictionaryEntries::new(a, &t).update_entries(&mut d);

            a.topology = Some(t.clone());
            a.dictionary = Some(d);
            for member in &def.members {
                let TopologyMember::SpecTlmPacketSet(node) = member else {
                    continue;
                };
                Self::add_packet_set(a, &t, node);
            }
            let d = a.dictionary.take().expect("dictionary under construction");
            a.topology = None;
            a.dictionary_map.insert(symbol, d);
        }
    }

    /// Construct a telemetry packet set and add it to the dictionary under
    /// construction.
    fn add_packet_set(a: &mut Analysis, t: &Topology, node: &SpecTlmPacketSet) {
        let mut d = a.dictionary.take().expect("dictionary under construction");
        match construct_packet_set(a, &d, t, node) {
            Ok(set) => {
                if let Err(err) = d.add_tlm_packet_set(set) {
                    err.emit();
                }
            }
            Err(err) => err.emit(),
        }
        a.dictionary = Some(d);
    }
}

/// Construct a telemetry packet set from its specifier.
fn construct_packet_set(
    a: &mut Analysis,
    d: &Dictionary,
    t: &Topology,
    node: &SpecTlmPacketSet,
) -> SemanticResult<TlmPacketSet> {
    a.tlm_packet_set = Some(TlmPacketSet::new(Arc::new(node.clone())));
    let result = add_packets(a, d, t, node);
    let set = a
        .tlm_packet_set
        .take()
        .expect("packet set under construction");
    result?;
    set.complete(a, d, t)
}

/// Add every packet of a packet set specifier to the packet set under
/// construction.
fn add_packets(
    a: &mut Analysis,
    d: &Dictionary,
    t: &Topology,
    node: &SpecTlmPacketSet,
) -> SemanticResult {
    for member in &node.members {
        let TlmPacketSetMember::SpecTlmPacket(packet_node) = member else {
            continue;
        };
        let id = a.get_nonnegative_big_int_value_opt(&packet_node.id)?;
        let packet = TlmPacket::from_spec_tlm_packet(a, d, t, packet_node)?;
        let mut set = a
            .tlm_packet_set
            .take()
            .expect("packet set under construction");
        let added = set.add_packet(id, packet);
        a.tlm_packet_set = Some(set);
        added?;
    }
    Ok(())
}
