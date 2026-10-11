use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::semantics::{ImpliedUse, ImpliedUseKind, ScopeOwner, Symbol};
use fpp_ast::{
    AstNode, MoveWalkable, Node, SpecTemplateExpand, TemplateArgKind, Visitable, Visitor,
};
use std::marker::PhantomData;
use std::ops::ControlFlow;

/// How far a [`NestedAnalyzer`] descends below the definitions with members
pub enum NestedAnalyzerMode {
    /// Visit translation unit, module, component and state machine members
    /// only, as the module, component and state machine analyzers do
    SHALLOW,
    /// Visit every node, as the use analyzers do
    DEEP,
    /// Visit every node, and visit the parameters bound by a template expansion
    /// before the members it introduces, as Scala's TypeExpressionAnalyzer
    /// (through TemplateExpandAnalyzer) does. A use analyzer is `DEEP`: it
    /// visits the bound parameters itself, through its template hooks
    /// ([`crate::analyzers::basic_use_analyzer::UseAnalysisPass::template_param`]).
    #[allow(non_camel_case_types)]
    TYPE_EXPRESSION,
}

/// A basic trait that keeps state of the symbol scope we are under
pub trait NestedScopeState {
    /// Look up a symbol given a node defining the symbol
    fn get_symbol<N: AstNode>(&self, node: &N) -> Option<Symbol>;

    /// Enter a scope under a symbol
    fn push_scope(&mut self, symbol: Symbol);

    /// Exit a scope
    fn pop_scope(&mut self);

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

/// Analyze the members of definitions, and in [`NestedAnalyzerMode::DEEP`]
/// mode types and expressions
///
/// The analyzer enters the scope of each component, module and state machine
/// it visits, which is the scope that use checking resolves names in.
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

    /// Analyze the parameters on a template expansion: the expression or type
    /// name bound to each constant or type parameter. A bound parameter holds a
    /// copy of its argument with the argument's node identity, so visiting the
    /// argument visits the bound value, whether or not the expansion exists yet.
    /// A port interface instance argument names no type or expression.
    fn visit_template_expand_params(
        &self,
        visitor: &V,
        a: &mut V::State,
        node: &'ast SpecTemplateExpand,
    ) -> ControlFlow<V::Break> {
        for arg in &node.args {
            match &arg.kind {
                TemplateArgKind::Constant(e) => e.visit(a, visitor)?,
                TemplateArgKind::Type(tn) => tn.visit(a, visitor)?,
                TemplateArgKind::Interface(_) => {}
            }
        }
        ControlFlow::Continue(())
    }

    fn walk_symbol(
        &self,
        visitor: &V,
        a: &mut V::State,
        symbol: Option<Symbol>,
        node: Node<'ast>,
    ) -> ControlFlow<V::Break> {
        if let Some(symbol) = symbol {
            a.push_scope(symbol);
            let out = node.walk(a, visitor);
            a.pop_scope();
            out
        } else {
            node.walk(a, visitor)
        }
    }
}

impl<'ast, S: NestedScopeState, V: Visitor<'ast, State = S>> Analyzer<'ast, V>
    for NestedAnalyzer<'ast, S, V>
{
    fn visit(&self, visitor: &V, a: &mut V::State, node: Node<'ast>) -> ControlFlow<V::Break> {
        match node {
            // Analyze component members
            Node::DefComponent(def) => self.walk_symbol(visitor, a, a.get_symbol(def), node),
            // Visit translation unit members and module members
            Node::DefModule(def) => self.walk_symbol(visitor, a, a.get_symbol(def), node),
            // Visit the members generated by an expanded template. They are
            // members of the enclosing module, so no scope is pushed here; the
            // expansion is recorded so that names inside it resolve against the
            // definitions the expansion introduced.
            Node::SpecTemplateExpand(def) => {
                if let NestedAnalyzerMode::TYPE_EXPRESSION = self.mode {
                    self.visit_template_expand_params(visitor, a, def)?;
                }
                match &def.members {
                    None => ControlFlow::Continue(()),
                    Some(members) => {
                        let saved = a.push_template_expansion(def.node_id);
                        let out = members.visit(a, visitor);
                        a.pop_template_expansion(saved);
                        out
                    }
                }
            }
            // Analyze state machine members
            Node::DefStateMachine(def) => self.walk_symbol(visitor, a, a.get_symbol(def), node),
            _ => match self.mode {
                NestedAnalyzerMode::SHALLOW => ControlFlow::Continue(()),
                NestedAnalyzerMode::DEEP | NestedAnalyzerMode::TYPE_EXPRESSION => {
                    node.walk(a, visitor)
                }
            },
        }
    }
}
