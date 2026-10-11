use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::ComponentInstance;
use fpp_ast::{self as ast, AstNode, ConnectionPatternKind, QualIdent};
use fpp_core::{Span, Spanned};
use std::sync::Arc;

/// An FPP connection pattern
#[derive(Debug, Clone)]
pub struct ConnectionPattern {
    /// The AST node specifying the pattern
    pub node: Arc<ast::SpecPatternConnectionGraph>,
    /// The source instance
    pub source: (ComponentInstance, Span),
    /// The target instances, in the order they are written. A target written
    /// twice appears twice; the duplicate connections it generates are dropped
    /// when the pattern is resolved.
    pub targets: Vec<(ComponentInstance, Span)>,
}

impl ConnectionPattern {
    /// Gets the location of the pattern
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Gets the kind of the pattern
    pub fn kind(&self) -> &ConnectionPatternKind {
        &self.node.kind
    }

    /// Constructs a connection pattern from an AST node
    ///
    /// Returns `None` if the source or a target names an instance that is
    /// unresolved or whose definition failed to check (both already reported).
    pub fn from_spec_connection_graph(
        a: &Analysis,
        node: &Arc<ast::SpecPatternConnectionGraph>,
    ) -> SemanticResult<Option<ConnectionPattern>> {
        let get_instance = |node: &QualIdent| -> SemanticResult<Option<(ComponentInstance, Span)>> {
            Ok(a.get_represented_component_instance(node.id())?
                .map(|instance| (instance, node.span())))
        };
        let Some(source) = get_instance(&node.source)? else {
            return Ok(None);
        };
        let mut targets = Vec::new();
        for target in &node.targets {
            let Some(target) = get_instance(target)? else {
                return Ok(None);
            };
            targets.push(target);
        }
        Ok(Some(ConnectionPattern {
            node: node.clone(),
            source,
            targets,
        }))
    }
}
