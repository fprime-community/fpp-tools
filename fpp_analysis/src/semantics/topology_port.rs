use crate::semantics::PortInstanceIdentifier;
use fpp_ast as ast;
use fpp_core::{Span, Spanned};
use std::sync::Arc;

/// An FPP topology port
#[derive(Debug, Clone)]
pub struct TopologyPort {
    /// The AST node specifying the topology port
    pub node: Arc<ast::SpecTopPort>,
    /// The underlying port instance identifier
    pub pii: PortInstanceIdentifier,
}

impl TopologyPort {
    /// Gets the name of the topology port
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the topology port
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Gets the location of the underlying port use
    pub fn get_underlying_port_loc(&self) -> Span {
        self.node.underlying_port.span()
    }
}
