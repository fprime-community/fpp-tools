use crate::semantics::Topology;
use fpp_ast::DefSystem;
use fpp_core::{Span, Spanned};
use std::sync::Arc;

/// An FPP system.
///
/// Records the system definition and the deployment topology it names.
#[derive(Debug, Clone)]
pub struct FppSystem {
    /// The AST node defining the system.
    pub node: Arc<DefSystem>,
    /// The deployment topology named by the system.
    pub topology: Topology,
}

impl FppSystem {
    /// Gets the name of the system.
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the system.
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }
}
