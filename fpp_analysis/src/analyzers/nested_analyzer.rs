use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::semantics::{ImpliedUseSet, Symbol};
use fpp_ast::{AstNode, MoveWalkable, Node, Visitor};
use std::marker::PhantomData;
use std::ops::ControlFlow;

pub enum NestedAnalyzerMode {
    SHALLOW,
    DEEP,
}

/// A basic trait that keeps state of the symbol scope we are under
pub trait NestedScopeState {
    /// Look up a symbol given a node defining the symbol
    fn get_symbol<N: AstNode>(&self, node: &N) -> Symbol;

    /// Enter a scope under a symbol
    fn push_scope(&mut self, symbol: Symbol);

    /// Exit a scope
    fn pop_scope(&mut self);

    /// Get the implied uses associated with an AST node, if any
    fn implied_uses(&self, node: fpp_core::Node) -> Option<ImpliedUseSet>;
}

impl NestedScopeState for Analysis {
    fn get_symbol<N: AstNode>(&self, node: &N) -> Symbol {
        self.get_symbol(node)
    }

    fn push_scope(&mut self, symbol: Symbol) {
        self.nested_scope.push(symbol)
    }

    fn pop_scope(&mut self) {
        self.nested_scope.pop()
    }

    fn implied_uses(&self, node: fpp_core::Node) -> Option<ImpliedUseSet> {
        self.implied_use_map.get(&node).cloned()
    }
}

pub struct NestedAnalyzer<'ast, S: NestedScopeState, V: Visitor<'ast, State = S>> {
    phantom_data: PhantomData<&'ast V>,
    mode: NestedAnalyzerMode,
}

impl<'ast, S: NestedScopeState, V: Visitor<'ast, State = S>> NestedAnalyzer<'ast, S, V> {
    pub fn new(mode: NestedAnalyzerMode) -> NestedAnalyzer<'ast, S, V> {
        NestedAnalyzer {
            phantom_data: Default::default(),
            mode,
        }
    }

    fn walk_symbol(
        &self,
        visitor: &V,
        a: &mut V::State,
        symbol: Symbol,
        node: Node<'ast>,
    ) -> ControlFlow<V::Break> {
        a.push_scope(symbol);
        let out = node.walk(a, visitor);
        a.pop_scope();
        out
    }
}

impl<'ast, S: NestedScopeState, V: Visitor<'ast, State = S>> Analyzer<'ast, V>
    for NestedAnalyzer<'ast, S, V>
{
    fn visit(&self, visitor: &V, a: &mut V::State, node: Node<'ast>) -> ControlFlow<V::Break> {
        match node {
            // Analyze component members
            Node::DefComponent(def) => self.walk_symbol(visitor, a, a.get_symbol(def), node),
            // Analyze enum constants
            Node::DefEnum(def) => self.walk_symbol(visitor, a, a.get_symbol(def), node),
            // Visit translation unit members and module members
            Node::DefModule(def) => self.walk_symbol(visitor, a, a.get_symbol(def), node),
            // Analyze state machine members
            Node::DefStateMachine(def) => self.walk_symbol(visitor, a, a.get_symbol(def), node),
            _ => match self.mode {
                NestedAnalyzerMode::SHALLOW => ControlFlow::Continue(()),
                NestedAnalyzerMode::DEEP => node.walk(a, visitor),
            },
        }
    }
}
