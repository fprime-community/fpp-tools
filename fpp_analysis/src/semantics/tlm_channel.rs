use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{Format, Type, Value};
use fpp_ast::{SpecTlmChannel, TlmChannelLimit, TlmChannelLimitKind, TlmChannelUpdate};
use fpp_core::{Node, Span, Spanned};
use rustc_hash::FxHashMap as HashMap;
use std::sync::Arc;

/// An FPP telemetry channel
#[derive(Debug, Clone)]
pub struct TlmChannel {
    pub node: Arc<SpecTlmChannel>,
    pub channel_type: Arc<Type>,
    pub update: TlmChannelUpdate,
    pub format: Option<Format>,
    pub low_limits: Limits,
    pub high_limits: Limits,
}

/// A telemetry channel ID
pub type TlmChannelId = i128;

/// A map from limit kinds to the node and value of the limit
pub type Limits = HashMap<TlmChannelLimitKind, (Node, Value)>;

impl TlmChannel {
    /// Gets the name of the channel
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the channel
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Creates a telemetry channel from a telemetry channel specifier
    pub fn from_spec_tlm_channel(
        a: &Analysis,
        node: &SpecTlmChannel,
    ) -> SemanticResult<TlmChannel> {
        let channel_type = a.get_finalized_type(node.type_name.node_id).unwrap();
        let update = node.update.clone().unwrap_or(TlmChannelUpdate::Always);
        // An invalid format string is reported, and the channel has no format
        let format = node.format.as_ref().and_then(|format| {
            Analysis::compute_format(format, std::slice::from_ref(&channel_type))
                .map_err(|err| err.emit())
                .ok()
        });
        let low_limits = compute_limits(a, &node.low)?;
        let high_limits = compute_limits(a, &node.high)?;
        a.check_displayable_type(
            node.type_name.node_id,
            node.type_name.span(),
            "type of telemetry channel is not displayable",
        )?;
        Ok(TlmChannel {
            node: Arc::new(node.clone()),
            channel_type,
            update,
            format,
            low_limits,
            high_limits,
        })
    }
}

/// Computes limits from AST limits
fn compute_limits(a: &Analysis, ast_limits: &[TlmChannelLimit]) -> SemanticResult<Limits> {
    let mut limits = Limits::default();
    // The kinds seen so far. Kept separately from `limits`, because a limit
    // whose expression has no value is still a duplicate of one that does.
    let mut seen: HashMap<TlmChannelLimitKind, Node> = HashMap::default();
    for limit in ast_limits {
        if let Some(prev_node) = seen.insert(limit.kind.clone(), limit.node_id) {
            return Err(SemanticError::DuplicateLimit {
                loc: limit.span(),
                prev_loc: prev_node.span(),
            });
        }
        // A limit expression with no value had its error reported by an
        // earlier pass; omit it from the map.
        if let Some(v) = a.value_map.get(&limit.value.node_id) {
            limits.insert(limit.kind.clone(), (limit.node_id, v.clone()));
        }
    }
    Ok(limits)
}
