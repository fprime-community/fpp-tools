use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{Symbol, SymbolInterface, TlmPacket, TlmPacketSet, Topology, cmp_span};
use fpp_ast::{SpecTlmPacketSet, TlmPacketSetMember, TopologyMember};
use rustc_hash::FxHashMap as HashMap;
use std::sync::Arc;

/// Construct and check the telemetry packet sets of every deployment topology.
///
/// The Scala implementation does this while constructing the dictionary
/// (`ConstructDictionaryMap`), which also supplies the global channel IDs.
/// Dictionary construction is not ported, so the channel IDs are computed
/// directly from the instance base IDs (see `semantics::tlm_packet_set`).
pub struct CheckTlmPacketSets;

impl CheckTlmPacketSets {
    pub fn check(a: &mut Analysis) {
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
            let mut set_map: HashMap<String, TlmPacketSet> = HashMap::default();
            for member in &def.members {
                let TopologyMember::SpecTlmPacketSet(node) = member else {
                    continue;
                };
                match construct_packet_set(a, &t, node) {
                    Ok(set) => {
                        let name = set.get_name().to_string();
                        match set_map.get(&name) {
                            Some(prev) => SemanticError::DuplicateTlmPacketSet {
                                name,
                                loc: set.get_loc(),
                                prev_loc: prev.get_loc(),
                            }
                            .emit(),
                            None => {
                                set_map.insert(name, set);
                            }
                        }
                    }
                    Err(err) => err.emit(),
                }
            }
            a.tlm_packet_set_map.insert(symbol, set_map);
        }
    }
}

/// Construct a telemetry packet set from its specifier.
fn construct_packet_set(
    a: &Analysis,
    t: &Topology,
    node: &SpecTlmPacketSet,
) -> SemanticResult<TlmPacketSet> {
    let mut set = TlmPacketSet::new(Arc::new(node.clone()));
    for member in &node.members {
        let TlmPacketSetMember::SpecTlmPacket(packet_node) = member else {
            continue;
        };
        let id = a.get_nonnegative_big_int_value_opt(&packet_node.id)?;
        let packet = TlmPacket::from_spec(a, t, packet_node)?;
        set.add_packet(id, packet)?;
    }
    set.complete(a, t)
}
