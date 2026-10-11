use crate::Analysis;
use crate::semantics::{ImpliedUse, ImpliedUseKind, ImpliedUseSet, ScopeOwner, Symbol};
use fpp_ast::{AstNode, Node, Visitor};
use std::ops::ControlFlow;

/// A generic analysis visitor
///
/// An analyzer is the super visitor of a pass: the hooks of the pass that do
/// not handle a node themselves defer to it, through
/// [`Visitor::super_visit`]. Analyzers compose by wrapping one another, each
/// one handling the nodes it analyzes and deferring the others to the analyzer
/// it wraps.
pub trait Analyzer<'ast, V: Visitor<'ast>> {
    /// Analyze a node, visiting the parts of it that the analysis covers
    fn visit(&self, visitor: &V, a: &mut V::State, node: Node<'ast>) -> ControlFlow<V::Break>;
}

/// The analyzer at the end of a chain of analyzers: it visits nothing below
/// the node it is given
#[derive(Default)]
pub struct DefaultAnalyzer;

impl<'ast, V: Visitor<'ast>> Analyzer<'ast, V> for DefaultAnalyzer {
    fn visit(&self, _: &V, _: &mut V::State, _: Node<'ast>) -> ControlFlow<V::Break> {
        ControlFlow::Continue(())
    }
}

/// Accessors of the analysis state that the analyzers and the passes built on
/// them read and update: the symbols of the definitions, the nested scope, the
/// implied uses, and the template expansion being analyzed
pub trait NestedScopeState {
    /// Look up a symbol given a node defining the symbol
    fn get_symbol<N: AstNode>(&self, node: &N) -> Option<Symbol>;

    /// Enter a scope under a symbol
    fn push_scope(&mut self, symbol: Symbol);

    /// Exit a scope
    fn pop_scope(&mut self);

    /// The implied uses associated with an AST node
    fn implied_uses(&self, node: fpp_core::Node) -> Option<ImpliedUseSet>;

    /// Get the implied uses of a kind associated with an AST node
    fn get_implied_uses(&self, kind: ImpliedUseKind, id: fpp_core::Node) -> Vec<ImpliedUse>;

    /// The bound parameters of the expansion at a template expansion specifier,
    /// or `None` if the specifier has not been expanded
    fn template_expansion_params(&self, node: fpp_core::Node) -> Option<Vec<Symbol>>;

    /// Enter the expansion at a template expansion specifier, returning the
    /// expansion that was current
    fn push_template_expansion(&mut self, node: fpp_core::Node) -> Option<fpp_core::Node>;

    /// Leave a template expansion, restoring the one that was current
    fn pop_template_expansion(&mut self, saved: Option<fpp_core::Node>);
}

impl NestedScopeState for Analysis {
    fn get_symbol<N: AstNode>(&self, node: &N) -> Option<Symbol> {
        self.get_symbol(node)
    }

    fn push_scope(&mut self, symbol: Symbol) {
        self.nested_scope.push(ScopeOwner::Symbol(symbol))
    }

    fn pop_scope(&mut self) {
        self.nested_scope.pop()
    }

    fn implied_uses(&self, node: fpp_core::Node) -> Option<ImpliedUseSet> {
        self.implied_use_map.get(&node).cloned()
    }

    fn get_implied_uses(&self, kind: ImpliedUseKind, id: fpp_core::Node) -> Vec<ImpliedUse> {
        Analysis::get_implied_uses(self, kind, id)
    }

    fn template_expansion_params(&self, node: fpp_core::Node) -> Option<Vec<Symbol>> {
        self.template_expansion_map
            .get(&node)
            .map(|expansion| expansion.params.clone())
    }

    fn push_template_expansion(&mut self, node: fpp_core::Node) -> Option<fpp_core::Node> {
        self.template_expansion.replace(node)
    }

    fn pop_template_expansion(&mut self, saved: Option<fpp_core::Node>) {
        self.template_expansion = saved;
    }
}
