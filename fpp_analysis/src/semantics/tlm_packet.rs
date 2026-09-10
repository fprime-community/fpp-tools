use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::{Topology, tlm_packet_set};
use fpp_ast::{SpecTlmPacket, TlmPacketMember};
use fpp_core::{Span, Spanned};
use rustc_hash::FxHashMap as HashMap;
use std::sync::Arc;

/// An FPP telemetry packet
#[derive(Debug, Clone)]
pub struct TlmPacket {
    /// The AST node for the packet
    pub node: Arc<SpecTlmPacket>,
    /// The packet group
    pub group: i32,
    /// The identifiers for the member channels
    pub member_id_list: Vec<i128>,
    /// The map from each member ID to a location where a member with that ID is
    /// specified. If more than one member has this ID, the map contains the
    /// last location.
    pub member_location_map: HashMap<i128, Span>,
}

impl TlmPacket {
    /// Gets the name of the packet
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the packet specifier
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Creates a telemetry packet from a telemetry packet specifier
    pub fn from_spec(
        a: &Analysis,
        t: &Topology,
        node: &SpecTlmPacket,
    ) -> SemanticResult<TlmPacket> {
        let group = a.get_nonnegative_int_value(node.group.node_id, node.group.span())?;
        let members: Vec<&fpp_ast::TlmChannelIdentifier> = node
            .members
            .iter()
            .filter_map(|m| match m {
                TlmPacketMember::TlmChannelIdentifier(node) => Some(node),
                TlmPacketMember::SpecInclude(_) => None,
            })
            .collect();
        let mut member_id_list = Vec::new();
        let mut member_location_map = HashMap::default();
        for member in members {
            let Some(id) = tlm_packet_set::get_numeric_id_for_node(a, t, member)? else {
                continue;
            };
            member_id_list.push(id);
            member_location_map.insert(id, member.span());
        }
        Ok(TlmPacket {
            node: Arc::new(node.clone()),
            group: group as i32,
            member_id_list,
            member_location_map,
        })
    }
}
