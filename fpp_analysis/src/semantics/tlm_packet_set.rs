use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    Dictionary, TlmChannelId, TlmChannelIdentifier, TlmPacket, TlmPacketId, Topology,
};
use fpp_ast::SpecTlmPacketSet;
use fpp_core::{Span, Spanned};
use indexmap::IndexMap;
use rustc_hash::{FxBuildHasher, FxHashMap as HashMap, FxHashSet as HashSet};
use std::sync::Arc;

/// An FPP telemetry packet set
#[derive(Debug, Clone)]
pub struct TlmPacketSet {
    /// The AST node
    pub node: Arc<SpecTlmPacketSet>,
    /// The map from packet IDs to packets
    pub packet_map: IndexMap<TlmPacketId, TlmPacket, FxBuildHasher>,
    /// The next default packet ID
    pub default_packet_id: TlmPacketId,
    /// The set of omitted channel IDs
    pub omitted_id_set: HashSet<TlmChannelId>,
    /// The map from each omitted channel ID to a location where the channel is
    /// marked as omitted. If the channel appears more than once in the omitted
    /// list in the source model, the map contains the last location.
    pub omitted_location_map: HashMap<TlmChannelId, Span>,
}

impl TlmPacketSet {
    /// Creates an empty packet set for a packet set specifier
    pub fn new(node: Arc<SpecTlmPacketSet>) -> TlmPacketSet {
        TlmPacketSet {
            node,
            packet_map: IndexMap::default(),
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
    pub fn add_packet(&mut self, id_opt: Option<TlmPacketId>, packet: TlmPacket) -> SemanticResult {
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
    pub fn get_used_id_set(&self) -> HashSet<TlmChannelId> {
        self.packet_map
            .values()
            .flat_map(|p| p.member_id_list.iter().copied())
            .collect()
    }

    /// Gets the used ID location map for the packet set
    pub fn get_used_id_location_map(&self) -> HashMap<TlmChannelId, Span> {
        self.packet_map
            .values()
            .flat_map(|p| p.member_location_map.iter().map(|(id, loc)| (*id, *loc)))
            .collect()
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
        let set_name = || self.get_name().to_string();
        let set_loc = self.get_loc();
        let check_id_used_or_omitted = |id: TlmChannelId| {
            if !used_id_set.contains(&id) && !self.omitted_id_set.contains(&id) {
                let entry = &d.tlm_channel_entry_map[&id];
                let channel_name = entry.get_qualified_name();
                let instance_loc = t
                    .look_up_component_instance_loc(&entry.instance)
                    .unwrap_or_else(|| {
                        panic!(
                            "component instance {} of telemetry channel {} is not an instance of topology {}",
                            entry.instance.get_qualified_name(),
                            channel_name,
                            t.get_name()
                        )
                    });
                let channel_loc = entry.tlm_channel.get_loc();
                Err(SemanticError::InvalidTlmPacketSet {
                    loc: set_loc,
                    name: set_name(),
                    msg: format!(
                        "telemetry channel {} is neither used nor marked as omitted",
                        channel_name
                    ),
                    notes: vec![
                        (
                            instance_loc,
                            "component instance is specified here".to_string(),
                        ),
                        (
                            channel_loc,
                            "telemetry channel is specified here".to_string(),
                        ),
                    ],
                })
            } else {
                Ok(())
            }
        };
        let check_id_not_used_and_omitted = |id: TlmChannelId| {
            if used_id_set.contains(&id) && self.omitted_id_set.contains(&id) {
                let entry = &d.tlm_channel_entry_map[&id];
                let channel_name = entry.get_qualified_name();
                let used_loc = used_id_location_map[&id];
                let omitted_loc = self.omitted_location_map[&id];
                Err(SemanticError::InvalidTlmPacketSet {
                    loc: set_loc,
                    name: set_name(),
                    msg: format!(
                        "telemetry channel {} is both used and marked omitted",
                        channel_name
                    ),
                    notes: vec![
                        (used_loc, "used here".to_string()),
                        (omitted_loc, "marked omitted here".to_string()),
                    ],
                })
            } else {
                Ok(())
            }
        };
        let check_id = |id: TlmChannelId| -> SemanticResult {
            check_id_used_or_omitted(id)?;
            check_id_not_used_and_omitted(id)
        };
        for id in d.tlm_channel_entry_map.keys() {
            check_id(*id)?;
        }
        Ok(())
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
}
