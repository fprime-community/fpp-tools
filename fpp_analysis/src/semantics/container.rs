use crate::Analysis;
use crate::errors::SemanticResult;
use fpp_ast::SpecContainer;
use fpp_core::{Span, Spanned};
use std::sync::Arc;

/// An FPP data product container
#[derive(Debug, Clone)]
pub struct Container {
    pub node: Arc<SpecContainer>,
    pub default_priority: Option<i128>,
}

/// A container ID
pub type ContainerId = i128;

impl Container {
    /// Gets the name of the container
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the container
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Creates a container from a container specifier
    pub fn from_spec_container(a: &Analysis, node: &SpecContainer) -> SemanticResult<Container> {
        let default_priority = a.get_nonnegative_big_int_value_opt(&node.default_priority)?;
        Ok(Container {
            node: Arc::new(node.clone()),
            default_priority,
        })
    }
}
