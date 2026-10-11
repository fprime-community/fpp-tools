use crate::Analysis;
use crate::errors::SemanticResult;
use fpp_ast::SpecInit;
use fpp_core::{Span, Spanned};
use std::sync::Arc;

/// An FPP init specifier
#[derive(Debug, Clone)]
pub struct InitSpecifier {
    pub node: Arc<SpecInit>,
    pub phase: i128,
}

impl InitSpecifier {
    /// Gets the location for this init specifier
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Creates an init specifier from an AST node
    pub fn from_node(a: &Analysis, node: &SpecInit) -> SemanticResult<InitSpecifier> {
        let phase = a.get_int_value_checked(node.phase.node_id, node.phase.span())?;
        Ok(InitSpecifier {
            node: Arc::new(node.clone()),
            phase,
        })
    }
}
