use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::UseAnalysisPass;
use crate::analyzers::use_analyzer::UseAnalyzer;
use crate::semantics::{QualifiedName, Symbol, Type};
use fpp_ast::*;
use rustc_hash::FxHashSet as HashSet;
use std::ops::ControlFlow;

/// Compute used symbols
///
/// There are two forms of resolution:
///
/// 1. Shallow resolution (don't follow uses from uses). This is used to
///    generate header files. You get this by calling a visitor method
///    of UsedSymbols.
///
/// 2. Deep resolution (follow uses from uses). This is used to generate
///    dictionary symbols. You get this by calling UsedSymbols::resolve_uses.
pub struct UsedSymbols<'ast> {
    super_: UseAnalyzer<'ast, Self>,
}

impl<'ast> Default for UsedSymbols<'ast> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> UsedSymbols<'ast> {
    pub fn new() -> UsedSymbols<'ast> {
        UsedSymbols {
            super_: UseAnalyzer::new(),
        }
    }

    // We could convert enum constant symbols to enum symbols here.
    // This would convert E.A to a use of E in shallow resolution.
    // Currently we don't do this, because we convert E.A to a numeric
    // constant in the generated code, so we don't need the dependency
    // on E.
    fn add_symbol(&self, a: &mut Analysis, node: fpp_core::Node) -> ControlFlow<()> {
        if let Some(symbol) = a.use_def_map.get(&node).cloned() {
            a.used_symbol_set.insert(symbol);
        }
        ControlFlow::Continue(())
    }

    /// Deep resolution of used symbols
    /// Replaces uses of enum constants with uses of the corresponding enums
    pub fn resolve_uses(&self, a: &mut Analysis, ss: &HashSet<Symbol>) -> HashSet<Symbol> {
        let mut out = HashSet::default();
        let mut visited = HashSet::default();
        for s in ss {
            self.resolve_node(a, s, &mut visited, &mut out);
        }
        out
    }

    fn resolve_node(
        &self,
        a: &mut Analysis,
        s: &Symbol,
        visited: &mut HashSet<Symbol>,
        out: &mut HashSet<Symbol>,
    ) {
        if !visited.insert(s.clone()) {
            return;
        }
        if let Some(resolved) = self.resolve_enum_constant(a, s) {
            out.insert(resolved);
        }
        let saved = std::mem::take(&mut a.used_symbol_set);
        let _ = self.visit_symbol(a, s);
        let shallow = std::mem::replace(&mut a.used_symbol_set, saved);
        for t in &shallow {
            self.resolve_node(a, t, visited, out);
        }
    }

    // When resolving uses, convert an enum constant symbol to the corresponding
    // enum symbol. For example, the use E.A becomes a use of E. This is what
    // we want, because E provides the definition of E.A.
    fn resolve_enum_constant(&self, a: &Analysis, s: &Symbol) -> Option<Symbol> {
        match s {
            Symbol::EnumConstant(node) => match a.type_map.get(&node.node_id).map(|t| &**t) {
                Some(Type::Enum(ty)) => Some(Symbol::EnumType(ty.node.clone())),
                _ => None,
            },
            _ => Some(s.clone()),
        }
    }

    fn visit_symbol(&self, a: &mut Analysis, s: &Symbol) -> ControlFlow<()> {
        match s.clone() {
            Symbol::AbsType(node) => node.visit(a, self),
            Symbol::AliasType(node) => node.visit(a, self),
            Symbol::ArrayType(node) => node.visit(a, self),
            Symbol::Component(node) => node.visit(a, self),
            Symbol::ComponentInstance(node) => node.visit(a, self),
            Symbol::Constant(node) => node.visit(a, self),
            Symbol::EnumType(node) => node.visit(a, self),
            Symbol::EnumConstant(node) => node.visit(a, self),
            Symbol::Interface(node) => node.visit(a, self),
            Symbol::Module(_) => ControlFlow::Continue(()),
            Symbol::Port(node) => node.visit(a, self),
            Symbol::StateMachine(node) => node.visit(a, self),
            Symbol::StructType(node) => node.visit(a, self),
            Symbol::System(node) => node.visit(a, self),
            Symbol::Topology(node) => node.visit(a, self),
        }
    }
}

impl<'ast> Visitor<'ast> for UsedSymbols<'ast> {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    // When resolving uses, if the default value of an enum definition is an enum
    // constant, then don't visit it. In this case the symbol is ignored (for
    // shallow resolution) or converted to a use of this enum (for deep resolution).
    // So it adds nothing to the resolution.
    fn visit_def_enum(&self, a: &mut Analysis, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        node.type_name.visit(a, self)?;
        node.constants.visit(a, self)?;
        match &node.default {
            Some(default) => match a.use_def_map.get(&default.node_id) {
                Some(Symbol::EnumConstant(_)) => ControlFlow::Continue(()),
                _ => default.visit(a, self),
            },
            None => ControlFlow::Continue(()),
        }
    }
}

impl<'ast> UseAnalysisPass<'ast, Analysis> for UsedSymbols<'ast> {
    fn component_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        _name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.add_symbol(a, node.id())
    }

    fn state_machine_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        _name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.add_symbol(a, node.id())
    }

    fn interface_instance_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        _name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.add_symbol(a, node.id())
    }

    fn constant_use(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        _name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.add_symbol(a, node.id())
    }

    fn interface_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        _name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.add_symbol(a, node.id())
    }

    fn type_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        _name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.add_symbol(a, node.id())
    }

    fn port_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        _name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.add_symbol(a, node.id())
    }

    fn implied_type_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        _name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.add_symbol(a, node.id())
    }

    fn implied_constant_use(
        &self,
        a: &mut Analysis,
        node: &Expr,
        _name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.add_symbol(a, node.id())
    }
}
