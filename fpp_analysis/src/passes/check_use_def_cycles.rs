use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{UseAnalysisPass, visit_template_arg};
use crate::analyzers::use_analyzer::UseAnalyzer;
use crate::errors::SemanticError;
use crate::semantics::{QualifiedName, Symbol, SymbolInterface, UseDefMatching};
use fpp_ast::*;
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

    fn error(&self, a: &Analysis, symbol: &Symbol) {
        SemanticError::UseDefCycle {
            loc: symbol.get_loc(),
            use_def_matching_list: a.use_def_matching_list.clone(),
        }
        .emit();
    }

    fn visit_def_pre(&self, a: &mut Analysis, symbol: Symbol) -> ControlFlow<()> {
        match symbol {
            Symbol::AliasType(node) => node.visit(a, self),
            Symbol::ArrayType(node) => node.visit(a, self),
            Symbol::Constant(node) => node.visit(a, self),
            Symbol::EnumType(node) => node.visit(a, self),
            Symbol::EnumConstant(node) => node.visit(a, self),
            Symbol::Interface(node) => node.visit(a, self),
            Symbol::StructType(node) => node.visit(a, self),
            Symbol::Topology(node) => node.visit(a, self),
            arg @ (Symbol::TemplateConstantArg { .. }
            | Symbol::TemplateTypeArg { .. }
            | Symbol::TemplateInterfaceArg { .. }) => self.template_param(a, &arg),
            _ => ControlFlow::Continue(()),
        }
    }

    /// Visit the definition of `symbol` with `visitor`, unless it is on the
    /// current use-def path, which closes a cycle, or it has been visited
    fn visit_def_post(
        &self,
        a: &mut Analysis,
        symbol: Symbol,
        visitor: impl FnOnce(&Self, &mut Analysis) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        if a.use_def_symbol_set.contains(&symbol) {
            self.error(a, &symbol);
            // A use-def cycle would make the later passes recurse forever, so
            // the analysis stops here
            ControlFlow::Break(())
        } else if !a.visited_symbol_set.contains(&symbol) {
            a.use_def_symbol_set.insert(symbol.clone());
            visitor(self, a)?;
            a.use_def_symbol_set.remove(&symbol);
            a.visited_symbol_set.insert(symbol);
            ControlFlow::Continue(())
        } else {
            ControlFlow::Continue(())
        }
    }

    fn visit_use(
        &self,
        a: &mut Analysis,
        node: fpp_core::Node,
        use_: QualifiedName,
    ) -> ControlFlow<()> {
        let symbol = match a.use_def_map.get(&node) {
            // The use did not resolve, which CheckUses reported
            None => return ControlFlow::Continue(()),
            Some(symbol) => symbol.clone(),
        };

        // The matching list holds the uses on the current use-def path
        let m = UseDefMatching {
            node_id: node,
            qualified_name: use_,
            symbol: symbol.clone(),
        };
        let len = a.use_def_matching_list.len();
        a.use_def_matching_list.push(m);
        let result = self.visit_def_pre(a, symbol);
        a.use_def_matching_list.truncate(len);
        result
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
        let symbol = a
            .get_symbol(node)
            .expect("definition should be entered by EnterSymbols");
        self.visit_def_post(a, symbol, |s, a| s.super_visit(a, Node::DefAliasType(node)))
    }

    fn visit_def_array(
        &self,
        a: &mut Self::State,
        node: &'ast DefArray,
    ) -> ControlFlow<Self::Break> {
        let symbol = a
            .get_symbol(node)
            .expect("definition should be entered by EnterSymbols");
        self.visit_def_post(a, symbol, |s, a| s.super_visit(a, Node::DefArray(node)))
    }

    fn visit_def_constant(
        &self,
        a: &mut Self::State,
        node: &'ast DefConstant,
    ) -> ControlFlow<Self::Break> {
        let symbol = a
            .get_symbol(node)
            .expect("definition should be entered by EnterSymbols");
        self.visit_def_post(a, symbol, |s, a| s.super_visit(a, Node::DefConstant(node)))
    }

    fn visit_def_enum(&self, a: &mut Self::State, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        let symbol = a
            .get_symbol(node)
            .expect("definition should be entered by EnterSymbols");
        self.visit_def_post(a, symbol, |s, a| s.super_visit(a, Node::DefEnum(node)))
    }

    fn visit_def_enum_constant(
        &self,
        a: &mut Self::State,
        node: &'ast DefEnumConstant,
    ) -> ControlFlow<Self::Break> {
        let symbol = a
            .get_symbol(node)
            .expect("definition should be entered by EnterSymbols");
        self.visit_def_post(a, symbol, |s, a| {
            s.super_visit(a, Node::DefEnumConstant(node))
        })
    }

    fn visit_def_struct(
        &self,
        a: &mut Self::State,
        node: &'ast DefStruct,
    ) -> ControlFlow<Self::Break> {
        let symbol = a
            .get_symbol(node)
            .expect("definition should be entered by EnterSymbols");
        self.visit_def_post(a, symbol, |s, a| s.super_visit(a, Node::DefStruct(node)))
    }

    fn visit_def_topology(
        &self,
        a: &mut Self::State,
        node: &'ast DefTopology,
    ) -> ControlFlow<Self::Break> {
        let symbol = a
            .get_symbol(node)
            .expect("definition should be entered by EnterSymbols");
        self.visit_def_post(a, symbol, |s, a| s.super_visit(a, Node::DefTopology(node)))
    }

    fn visit_def_interface(
        &self,
        a: &mut Self::State,
        node: &'ast DefInterface,
    ) -> ControlFlow<Self::Break> {
        let symbol = a
            .get_symbol(node)
            .expect("definition should be entered by EnterSymbols");
        self.visit_def_post(a, symbol, |s, a| s.super_visit(a, Node::DefInterface(node)))
    }
}

impl<'ast> UseAnalysisPass<'ast> for CheckUseDefCycles<'ast> {
    fn constant_use(
        &self,
        a: &mut Analysis,
        node: &Expr,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.visit_use(a, node.id(), use_)
    }

    fn template_constant_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        let arg = arg.clone();
        self.visit_def_post(a, arg.clone(), |s, a| visit_template_arg(s, a, &arg))
    }

    fn template_type_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        let arg = arg.clone();
        self.visit_def_post(a, arg.clone(), |s, a| visit_template_arg(s, a, &arg))
    }

    fn template_interface_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        let arg = arg.clone();
        self.visit_def_post(a, arg.clone(), |s, a| visit_template_arg(s, a, &arg))
    }

    fn interface_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.visit_use(a, node.id(), use_)
    }

    fn interface_instance_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.visit_use(a, node.id(), use_)
    }

    fn type_use(
        &self,
        a: &mut Analysis,
        node: &TypeName,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.visit_use(a, node.node_id, use_)
    }
}
