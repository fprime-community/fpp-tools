use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{Dictionary, TlmChannelIdentifier, TlmPacket, Topology};
use fpp_ast::SpecTlmPacketSet;
use fpp_core::{Span, Spanned};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::sync::Arc;

/// An FPP telemetry packet set
#[derive(Debug, Clone)]
pub struct TlmPacketSet {
    /// The AST node
    pub node: Arc<SpecTlmPacketSet>,
    /// The map from packet IDs to packets
    pub packet_map: HashMap<i128, TlmPacket>,
    /// The next default packet ID
    pub default_packet_id: i128,
    /// The set of omitted channel IDs
    pub omitted_id_set: HashSet<i128>,
    /// The map from each omitted channel ID to a location where the channel is
    /// marked as omitted. If the channel appears more than once in the omitted
    /// list in the source model, the map contains the last location.
    pub omitted_location_map: HashMap<i128, Span>,
}

impl TlmPacketSet {
    pub fn new(node: Arc<SpecTlmPacketSet>) -> TlmPacketSet {
        TlmPacketSet {
            node,
            packet_map: HashMap::default(),
            default_packet_id: 0,
            omitted_id_set: HashSet::default(),
            omitted_location_map: HashMap::default(),
        }
    }

    /// Gets the name of the packet set
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the packet set specifier
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Add a telemetry packet to the set
    pub fn add_packet(&mut self, id_opt: Option<i128>, packet: TlmPacket) -> SemanticResult {
        let next = Analysis::add_element_to_id_map(
            &mut self.packet_map,
            id_opt.unwrap_or(self.default_packet_id),
            packet,
            TlmPacket::get_loc,
        )?;
        self.default_packet_id = next;
        Ok(())
    }

    /// Gets the channels used in the packet set
    pub fn get_used_id_set(&self) -> HashSet<i128> {
        self.packet_map
            .values()
            .flat_map(|p| p.member_id_list.iter().copied())
            .collect()
    }

    /// Gets the used ID location map for the packet set
    pub fn get_used_id_location_map(&self) -> HashMap<i128, Span> {
        self.packet_map
            .values()
            .flat_map(|p| p.member_location_map.iter().map(|(id, loc)| (*id, *loc)))
            .collect()
    }

    /// Completes a telemetry packet set definition
    pub fn complete(
        mut self,
        a: &Analysis,
        d: &Dictionary,
        t: &Topology,
    ) -> SemanticResult<TlmPacketSet> {
        Analysis::check_dictionary_names(
            &self.packet_map,
            "packet",
            |p| p.get_name().to_string(),
            TlmPacket::get_loc,
        )?;
        self.compute_omitted_channels(a, d, t)?;
        self.check_channel_usage(d, t)?;
        Ok(self)
    }

    /// Computes the omitted channels of the packet set
    fn compute_omitted_channels(
        &mut self,
        a: &Analysis,
        d: &Dictionary,
        t: &Topology,
    ) -> SemanticResult {
        for node in &self.node.omitted.clone() {
            let Some(id) = TlmChannelIdentifier::get_numeric_id_for_node(a, d, t, node)? else {
                continue;
            };
            self.omitted_id_set.insert(id);
            self.omitted_location_map.insert(id, node.span());
        }
        Ok(())
    }

    /// Checks that each channel is either used or omitted, but not both
    fn check_channel_usage(&self, d: &Dictionary, t: &Topology) -> SemanticResult {
        let used_id_set = self.get_used_id_set();
        let used_id_location_map = self.get_used_id_location_map();
        let set_name = self.get_name().to_string();
        let set_loc = self.get_loc();
        for (id, entry) in &d.tlm_channel_entry_map {
            let id = *id;
            if !used_id_set.contains(&id) && !self.omitted_id_set.contains(&id) {
                let instance_loc = t
                    .look_up_component_instance_loc(&entry.instance)
                    .expect("channel entry instance is an instance of the topology");
                return Err(SemanticError::InvalidTlmPacketSetChannel {
                    loc: set_loc,
                    name: set_name,
                    msg: format!(
                        "telemetry channel {} is neither used nor marked as omitted",
                        entry.get_qualified_name()
                    ),
                    notes: vec![
                        (
                            instance_loc,
                            "component instance is specified here".to_string(),
                        ),
                        (
                            entry.tlm_channel.get_loc(),
                            "telemetry channel is specified here".to_string(),
                        ),
                    ],
                });
            }
            if used_id_set.contains(&id) && self.omitted_id_set.contains(&id) {
                return Err(SemanticError::InvalidTlmPacketSetChannel {
                    loc: set_loc,
                    name: set_name,
                    msg: format!(
                        "telemetry channel {} is both used and marked omitted",
                        entry.get_qualified_name()
                    ),
                    notes: vec![
                        (used_id_location_map[&id], "used here".to_string()),
                        (
                            self.omitted_location_map[&id],
                            "marked omitted here".to_string(),
                        ),
                    ],
                });
            }
        }
        Ok(())
    }
}
