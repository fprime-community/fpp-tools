use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::component::{add_element_to_id_map, check_dictionary_names};
use crate::semantics::{ComponentInstance, TlmChannel, TlmChannelIdentifier, TlmPacket, Topology};
use fpp_ast::{self as ast, SpecTlmPacketSet};
use fpp_core::{Span, Spanned};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::collections::BTreeMap;
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
        let next = add_element_to_id_map(
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
    pub fn complete(mut self, a: &Analysis, t: &Topology) -> SemanticResult<TlmPacketSet> {
        check_dictionary_names(
            &self.packet_map,
            "packet",
            |p| p.get_name().to_string(),
            TlmPacket::get_loc,
        )?;
        self.compute_omitted_channels(a, t)?;
        self.check_channel_usage(a, t)?;
        Ok(self)
    }

    /// Computes the omitted channels of the packet set
    fn compute_omitted_channels(&mut self, a: &Analysis, t: &Topology) -> SemanticResult {
        for node in &self.node.omitted.clone() {
            let Some(id) = get_numeric_id_for_node(a, t, node)? else {
                continue;
            };
            self.omitted_id_set.insert(id);
            self.omitted_location_map.insert(id, node.span());
        }
        Ok(())
    }

    /// Checks that each channel is either used or omitted, but not both
    fn check_channel_usage(&self, a: &Analysis, t: &Topology) -> SemanticResult {
        let used_id_set = self.get_used_id_set();
        let used_id_location_map = self.get_used_id_location_map();
        let set_name = self.get_name().to_string();
        let set_loc = self.get_loc();
        for (id, entry) in tlm_channel_entry_map(a, t) {
            if !used_id_set.contains(&id) && !self.omitted_id_set.contains(&id) {
                let instance_loc = t
                    .look_up_component_instance_loc(&entry.0)
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
                            entry.1.get_loc(),
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

/// A telemetry channel entry of a topology: the component instance that
/// provides the channel, and the channel itself.
///
/// The Scala implementation stores these in `Dictionary.tlmChannelEntryMap`,
/// built by `ConstructDictionary`. Dictionary construction is not ported, so
/// the entries are computed on demand here.
pub struct TlmChannelEntry(pub ComponentInstance, pub TlmChannel);

impl TlmChannelEntry {
    /// The qualified name of the entry: instance name + channel name.
    pub fn get_qualified_name(&self) -> String {
        format!("{}.{}", self.0.get_qualified_name(), self.1.get_name())
    }
}

/// The map from global telemetry channel IDs to channel entries, for every
/// component instance of the topology. A global ID is the instance base ID plus
/// the channel's ID within its component.
fn tlm_channel_entry_map(a: &Analysis, t: &Topology) -> BTreeMap<i128, TlmChannelEntry> {
    let mut result = BTreeMap::new();
    for ci in t.component_instance_map().into_keys() {
        let Some(component) = ci.get_component(a) else {
            continue;
        };
        for (local_id, channel) in &component.tlm_channel_map {
            result.insert(
                ci.base_id + local_id,
                TlmChannelEntry(ci.clone(), channel.clone()),
            );
        }
    }
    result
}

/// Gets the global telemetry channel ID named by an AST channel identifier.
/// Returns `None` if the component instance is unresolved (already reported by
/// CheckUses).
pub fn get_numeric_id_for_node(
    a: &Analysis,
    t: &Topology,
    node: &ast::TlmChannelIdentifier,
) -> SemanticResult<Option<i128>> {
    let Some(channel_id) = TlmChannelIdentifier::from_node(a, node)? else {
        return Ok(None);
    };
    find_numeric_id_for_channel(a, t, &channel_id).map(Some)
}

/// Finds the global ID of the channel named by a telemetry channel identifier.
fn find_numeric_id_for_channel(
    a: &Analysis,
    t: &Topology,
    channel_id: &TlmChannelIdentifier,
) -> SemanticResult<i128> {
    let ci = &channel_id.component_instance;
    let name = channel_id.tlm_channel.get_name();
    if t.look_up_component_instance_loc(ci).is_some()
        && let Some(component) = ci.get_component(a)
        && let Some((local_id, _)) = component
            .tlm_channel_map
            .iter()
            .find(|(_, channel)| channel.get_name() == name)
    {
        return Ok(ci.base_id + local_id);
    }
    Err(SemanticError::ChannelNotInDictionary {
        loc: channel_id.get_loc(),
        channel_name: channel_id.get_qualified_name(),
        top_name: t.get_name().to_string(),
    })
}
