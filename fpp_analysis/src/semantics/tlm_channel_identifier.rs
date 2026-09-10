use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::{ComponentInstance, TlmChannel};
use fpp_ast::{self as ast, AstNode};
use fpp_core::{Span, Spanned};
use std::sync::Arc;

/// An FPP telemetry channel instance identifier
#[derive(Debug, Clone)]
pub struct TlmChannelIdentifier {
    /// The AST node
    pub node: Arc<ast::TlmChannelIdentifier>,
    /// The component instance
    pub component_instance: ComponentInstance,
    /// The telemetry channel
    pub tlm_channel: TlmChannel,
}

impl std::fmt::Display for TlmChannelIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.get_qualified_name())
    }
}

impl TlmChannelIdentifier {
    /// Gets the location of the telemetry channel identifier
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Gets the qualified name
    pub fn get_qualified_name(&self) -> String {
        format!(
            "{}.{}",
            self.component_instance.get_qualified_name(),
            self.tlm_channel.get_name()
        )
    }

    /// Gets the unqualified name
    pub fn get_unqualified_name(&self) -> String {
        format!(
            "{}.{}",
            self.component_instance.get_unqualified_name(),
            self.tlm_channel.get_name()
        )
    }

    /// Creates a telemetry channel identifier from an AST node. Returns `None`
    /// if the component instance is unresolved (already reported by CheckUses).
    pub fn from_node(
        a: &Analysis,
        node: &ast::TlmChannelIdentifier,
    ) -> SemanticResult<Option<TlmChannelIdentifier>> {
        let Some(component_instance) = a.get_component_instance(node.component_instance.id())?
        else {
            return Ok(None);
        };
        let Some(component) = component_instance.get_component(a) else {
            return Ok(None);
        };
        let tlm_channel = component.get_tlm_channel_by_name(&node.channel_name)?;
        Ok(Some(TlmChannelIdentifier {
            node: Arc::new(node.clone()),
            component_instance,
            tlm_channel,
        }))
    }
}
