use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::UseAnalysisPass;
use crate::analyzers::use_analyzer::UseAnalyzer;
use crate::errors::SemanticError;
use crate::semantics::{QualifiedName, Symbol, SymbolInterface, UseDefMatching};
use fpp_ast::*;
use fpp_core::Spanned;
use std::ops::ControlFlow;

/// Check for use-def cycles
pub struct CheckUseDefCycles<'ast> {
    super_: UseAnalyzer<'ast, Self>,
}

impl<'ast> Default for CheckUseDefCycles<'ast> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> CheckUseDefCycles<'ast> {
    pub fn new() -> CheckUseDefCycles<'ast> {
        Self {
            super_: UseAnalyzer::new(),
        }
    }

    fn visit_def_pre(&self, a: &mut Analysis, symbol: Symbol) -> ControlFlow<()> {
        match symbol {
            Symbol::AliasType(node) => node.visit(a, self),
            Symbol::Array(node) => node.visit(a, self),
            Symbol::Constant(node) => node.visit(a, self),
            Symbol::Enum(node) => node.visit(a, self),
            Symbol::EnumConstant(node) => node.visit(a, self),
            Symbol::Interface(node) => node.visit(a, self),
            Symbol::Struct(node) => node.visit(a, self),
            Symbol::Topology(node) => node.visit(a, self),
            _ => ControlFlow::Continue(()),
        }
    }

    fn visit_use(
        &self,
        a: &mut Analysis,
        node: fpp_core::Node,
        use_name: QualifiedName,
    ) -> ControlFlow<()> {
        let symbol = match a.use_def_map.get(&node) {
            // CheckUses failed in the last pass, don't do any more analysis
            None => return ControlFlow::Continue(()),
            Some(symbol) => symbol.clone(),
        };

        let m = UseDefMatching {
            node,
            qualified_name: use_name,
            symbol: symbol.clone(),
        };

        // The extended matching list must be scoped to the current use-def
        // path: push `m`, recurse into the visited subtree, then truncate back
        // to the saved length on the way out — otherwise the list accretes
        // entries from every resolved use in the translation unit and pollutes
        // the `UseDefCycle` diagnostic.
        let len = a.use_def_matching_list.len();
        a.use_def_matching_list.push(m);
        let result = self.visit_def_pre(a, symbol);
        a.use_def_matching_list.truncate(len);
        result
    }

    fn visit_def_post<T: Walkable<'ast, Self>>(
        &self,
        a: &mut Analysis,
        symbol: Symbol,
        node: &'ast T,
    ) -> ControlFlow<()> {
        if a.use_def_symbol_set.contains(&symbol) {
            SemanticError::UseDefCycle {
                loc: symbol.node().span(),
                cycle: a.use_def_matching_list.iter().map(|m| m.into()).collect(),
            }
            .emit();

            // This is one of the rare cases where we need to stop analysis
            // This pass checks for use-def cycles which will cause infinite recursion
            // in later stages of the analysis.
            ControlFlow::Break(())
        } else if !a.visited_symbol_set.contains(&symbol) {
            a.use_def_symbol_set.insert(symbol.clone());
            node.walk(a, self)?;
            a.use_def_symbol_set.remove(&symbol);
            a.visited_symbol_set.insert(symbol);
            ControlFlow::Continue(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

impl<'ast> Visitor<'ast> for CheckUseDefCycles<'ast> {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    fn visit_def_alias_type(
        &self,
        a: &mut Self::State,
        node: &'ast DefAliasType,
    ) -> ControlFlow<Self::Break> {
        let symbol = a.get_symbol(node);
        self.visit_def_post(a, symbol, node)
    }

    fn visit_def_array(
        &self,
        a: &mut Self::State,
        node: &'ast DefArray,
    ) -> ControlFlow<Self::Break> {
        let symbol = a.get_symbol(node);
        self.visit_def_post(a, symbol, node)
    }

    fn visit_def_constant(
        &self,
        a: &mut Self::State,
        node: &'ast DefConstant,
    ) -> ControlFlow<Self::Break> {
        let symbol = a.get_symbol(node);
        self.visit_def_post(a, symbol, node)
    }

    fn visit_def_enum(&self, a: &mut Self::State, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        let symbol = a.get_symbol(node);
        self.visit_def_post(a, symbol, node)
    }

    fn visit_def_enum_constant(
        &self,
        a: &mut Self::State,
        node: &'ast DefEnumConstant,
    ) -> ControlFlow<Self::Break> {
        let symbol = a.get_symbol(node);
        self.visit_def_post(a, symbol, node)
    }

    fn visit_def_struct(
        &self,
        a: &mut Self::State,
        node: &'ast DefStruct,
    ) -> ControlFlow<Self::Break> {
        let symbol = a.get_symbol(node);
        self.visit_def_post(a, symbol, node)
    }

    fn visit_def_topology(
        &self,
        a: &mut Self::State,
        node: &'ast DefTopology,
    ) -> ControlFlow<Self::Break> {
        let symbol = a.get_symbol(node);
        self.visit_def_post(a, symbol, node)
    }

    fn visit_def_interface(
        &self,
        a: &mut Self::State,
        node: &'ast DefInterface,
    ) -> ControlFlow<Self::Break> {
        let symbol = a.get_symbol(node);
        self.visit_def_post(a, symbol, node)
    }
}

impl<'ast> UseAnalysisPass<'ast, Analysis> for CheckUseDefCycles<'ast> {
    fn interface_instance_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.visit_use(a, node.id(), name)
    }

    fn constant_use(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.visit_use(a, node.id(), name)
    }

    fn interface_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.visit_use(a, node.id(), name)
    }

    fn type_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.visit_use(a, node.id(), name)
    }

    fn implied_type_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.visit_use(a, node.id(), name)
    }

    fn implied_constant_use(
        &self,
        a: &mut Analysis,
        node: &Expr,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.visit_use(a, node.id(), name)
    }
}
