//! Giving a copied AST subtree its own node identities.
//!
//! A node's identity is its [`fpp_core::Node`] handle: the compiler context maps
//! that handle to a span, and the analysis maps it to types, values and symbols.
//! Copying a subtree with `Clone` copies the handles too, so the copy and the
//! original would be the same nodes as far as every map is concerned. Walking
//! the copy with a [`Replicator`] gives each node in it a fresh handle whose span
//! points back at the source text, so a diagnostic still reports the original
//! location and the two copies stay distinct everywhere else.

use fpp_core::{Node, Span, Spanned};

/// Replaces the identity of every node in a copied AST subtree.
pub trait Replicate {
    fn replicate(&mut self, r: &Replicator);
}

/// Assigns fresh identities to the nodes of a copied subtree.
pub struct Replicator {
    /// The span of the template expansion specifier that made the copy, recorded
    /// on every replicated span so diagnostics can report the expansion point
    expand_span: Span,
}

impl Replicator {
    /// A replicator for a copy made at the template expansion specifier spanning
    /// `expand_span`
    pub fn new(expand_span: Span) -> Replicator {
        Replicator { expand_span }
    }

    /// The span to give a copy of the source text at `span`
    pub fn span(&self, span: Span) -> Span {
        span.expanded_at(self.expand_span)
    }

    /// The identity to give a copy of the node at `node`
    pub fn node(&self, node: Node) -> Node {
        let out = Node::new(self.span(node.span()));
        Node::annotate(
            &out,
            fpp_core::Annotated::pre_annotation(&node),
            fpp_core::Annotated::post_annotation(&node),
        );
        out
    }

    /// Replicate the nodes of `subtree` in place
    pub fn apply<T: Replicate>(&self, subtree: &mut T) {
        subtree.replicate(self);
    }
}

impl Replicate for Node {
    fn replicate(&mut self, r: &Replicator) {
        *self = r.node(*self);
    }
}

impl Replicate for Span {
    fn replicate(&mut self, r: &Replicator) {
        *self = r.span(*self);
    }
}

impl<T: Replicate> Replicate for Option<T> {
    fn replicate(&mut self, r: &Replicator) {
        if let Some(inner) = self {
            inner.replicate(r);
        }
    }
}

impl<T: Replicate> Replicate for Vec<T> {
    fn replicate(&mut self, r: &Replicator) {
        for item in self {
            item.replicate(r);
        }
    }
}

impl<T: Replicate> Replicate for Box<T> {
    fn replicate(&mut self, r: &Replicator) {
        (**self).replicate(r);
    }
}

/// The leaf types of the grammar carry no node identity, so replicating one is a
/// no-op.
macro_rules! replicate_leaves {
    ($($ty:ty,)*) => {
        $(
            impl Replicate for $ty {
                fn replicate(&mut self, _r: &Replicator) {}
            }
        )*
    };
}

replicate_leaves! {
    bool,
    String,
    crate::Binop,
    crate::ComponentKind,
    crate::ConnectionPatternKind,
    crate::EventSeverity,
    crate::FloatKind,
    crate::FormalParamKind,
    crate::GeneralPortInstanceKind,
    crate::InputPortKind,
    crate::IntegerKind,
    crate::QueueFull,
    crate::SpecLocKind,
    crate::SpecialPortInstanceKind,
    crate::TlmChannelLimitKind,
    crate::TlmChannelUpdate,
    crate::Unop,
}
