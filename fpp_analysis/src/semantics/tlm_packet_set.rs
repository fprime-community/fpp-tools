use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{Dictionary, TlmChannelEntry, TlmChannelIdentifier, TlmPacket, Topology};
use fpp_ast::{SpecTlmPacketSet, TlmPacketMember, TlmPacketSetMember};
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
        self.node.name.span()
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
        self.check_channel_usage(d)?;
        Ok(self)
    }

    /// Gets the dictionary channels whose IDs are not in `covered_id_set`, in
    /// channel ID order
    pub fn get_uncovered_channels<'d>(
        d: &'d Dictionary,
        covered_id_set: &HashSet<i128>,
    ) -> Vec<&'d TlmChannelEntry> {
        d.tlm_channel_entry_map
            .iter()
            .filter(|(id, _)| !covered_id_set.contains(id))
            .map(|(_, entry)| entry)
            .collect()
    }

    /// Gets the channels that the packet set specifier `node` neither uses nor
    /// marks as omitted, in channel ID order.
    ///
    /// Constructing a packet set discards it when a channel is left uncovered,
    /// so a tool that wants the uncovered channels after the fact (an editor
    /// quick fix, say) has to recover them from the specifier. Channel
    /// identifiers that name no channel of the topology are skipped: they are
    /// reported by their own diagnostic.
    pub fn get_uncovered_channels_for_node<'d>(
        a: &Analysis,
        d: &'d Dictionary,
        t: &Topology,
        node: &SpecTlmPacketSet,
    ) -> Vec<&'d TlmChannelEntry> {
        let channel_nodes = node
            .members
            .iter()
            .filter_map(|member| match member {
                TlmPacketSetMember::SpecTlmPacket(packet) => Some(packet),
                TlmPacketSetMember::SpecInclude(_) => None,
            })
            .flat_map(|packet| &packet.members)
            .filter_map(|member| match member {
                TlmPacketMember::TlmChannelIdentifier(node) => Some(node),
                TlmPacketMember::SpecInclude(_) => None,
            })
            .chain(&node.omitted);
        let covered_id_set = channel_nodes
            .filter_map(|node| {
                TlmChannelIdentifier::get_numeric_id_for_node(a, d, t, node).unwrap_or(None)
            })
            .collect();
        Self::get_uncovered_channels(d, &covered_id_set)
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

    /// Checks that each channel is either used or omitted, but not both.
    ///
    /// Each check reports every offending channel in one diagnostic: a topology
    /// that forgets a packet set member typically forgets many, and one error
    /// per channel would bury the rest of the model's diagnostics.
    fn check_channel_usage(&self, d: &Dictionary) -> SemanticResult {
        let used_id_set = self.get_used_id_set();
        let covered_id_set = used_id_set.union(&self.omitted_id_set).copied().collect();
        let uncovered_channels = Self::get_uncovered_channels(d, &covered_id_set);
        if !uncovered_channels.is_empty() {
            return Err(SemanticError::TlmPacketSetChannelsNotCovered {
                loc: self.get_loc(),
                name: self.get_name().to_string(),
                channels: uncovered_channels
                    .iter()
                    .map(|entry| entry.get_qualified_name())
                    .collect(),
            });
        }

        let used_id_location_map = self.get_used_id_location_map();
        let used_and_omitted_channels: Vec<_> = d
            .tlm_channel_entry_map
            .iter()
            .filter(|(id, _)| used_id_set.contains(id) && self.omitted_id_set.contains(id))
            .map(|(id, entry)| {
                (
                    entry.get_qualified_name(),
                    used_id_location_map[id],
                    self.omitted_location_map[id],
                )
            })
            .collect();
        if !used_and_omitted_channels.is_empty() {
            return Err(SemanticError::TlmPacketSetChannelsUsedAndOmitted {
                loc: self.get_loc(),
                name: self.get_name().to_string(),
                channels: used_and_omitted_channels,
            });
        }
        Ok(())
    }
}
