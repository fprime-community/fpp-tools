use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{BasicUseAnalyzer, UseAnalysisPass, visit_template_arg};
use crate::errors::SemanticError;
use crate::passes::CheckUsesHelpers;
use crate::semantics::{
    ImpliedUse, ImpliedUseKind, NameGroup, NameGroupMap, NameSymbolMap, QualifiedName, ScopeOwner,
    Symbol, SymbolInterface,
};
use fpp_ast::{
    AstNode, DefComponent, DefEnum, DefModule, DefStateMachine, Expr, ExprKind, Ident, Node,
    QualIdent, SpecTemplateExpand, TypeName, TypeNameKind, Visitable, Visitor,
};
use fpp_core::Spanned;
use std::ops::ControlFlow;

/// Match uses to their definitions
pub struct CheckUses<'ast> {
    super_: BasicUseAnalyzer<'ast, Self>,
}

impl<'ast> Default for CheckUses<'ast> {
    fn default() -> Self {
        Self::new()
    }
}

/// The check uses helpers for the analysis.
///
/// The scope of a symbol is [`Analysis::lookup_scope`], which includes the
/// symbols defined by the template expansion being analyzed.
pub(crate) const HELPERS: CheckUsesHelpers<
    Analysis,
    NameGroup,
    Symbol,
    NameGroupMap<NameSymbolMap>,
> = CheckUsesHelpers {
    get_nested_scope_symbol: Analysis::nested_scope_get,
    get_symbol_scope: Analysis::lookup_scope,
    get_use_def_map: |a| &mut a.use_def_map,
};

/// Visit a qualified identifier node in name group `ng` and check the use,
/// emitting any error
pub(crate) fn check_qual_ident(a: &mut Analysis, ng: NameGroup, node: &QualIdent) {
    if let Err(err) = HELPERS.visit_qual_ident_node(ng, a, node) {
        err.emit();
    }
}

impl<'ast> CheckUses<'ast> {
    pub fn new() -> CheckUses<'ast> {
        Self {
            super_: BasicUseAnalyzer::new(),
        }
    }

    fn visit_expr_ident(&self, a: &mut Analysis, node: &Expr, name: &str, is_absolute: bool) {
        let mapping = |name: &str| a.nested_scope_get(is_absolute, NameGroup::Value, name);
        match HELPERS.get_symbol_for_name(NameGroup::Value, mapping, node.id(), name) {
            Ok(symbol) => {
                a.use_def_map.insert(node.id(), symbol);
            }
            Err(err) => err.emit(),
        }
    }

    fn visit_expr_dot(
        &self,
        a: &mut Analysis,
        node: &Expr,
        e: &Expr,
        id: &Ident,
    ) -> ControlFlow<()> {
        // Visit the left side of the dot recursively
        e.visit(a, self)?;

        // Find the symbol referred to by the left side (if-any)
        let symbol = match a.use_def_map.get(&e.id()) {
            // Left side is a constant, we are selecting a member of this constant
            // we are not creating another use.
            Some(Symbol::Constant(_) | Symbol::TemplateConstantArg { .. }) => Ok(None),

            // The left side is some symbol other than a constant (a qualifier),
            // look up this symbol and add it to the use-def entries
            Some(qual) => match a.lookup_scope(qual) {
                Some(scope) => {
                    let mapping = |name: &str| scope.get(NameGroup::Value, name);
                    HELPERS
                        .get_symbol_for_name(NameGroup::Value, mapping, id.id(), &id.data)
                        .map(Some)
                }
                // The left side is a value with no scope (an enumerated
                // constant), so the dot selects a member of the value
                None => Ok(None),
            },

            // Left-hand side is not a symbol
            // There is no resolution on the dot expression
            None => Ok(None),
        };

        match symbol {
            Ok(Some(symbol)) => {
                a.use_def_map.insert(node.id(), symbol);
            }
            Ok(None) => {}
            Err(err) => {
                err.emit();
                // A use that does not resolve has no definition. The nodes of
                // an implied use share one id, so the entry the left side
                // recorded would otherwise stand for the whole use.
                a.use_def_map.remove(&node.id());
            }
        }
        ControlFlow::Continue(())
    }

    /// Visit a definition with members in the scope that it opens
    fn visit_in_scope<N: AstNode>(
        &self,
        a: &mut Analysis,
        def: &N,
        node: Node<'ast>,
    ) -> ControlFlow<()> {
        let symbol = a
            .get_symbol(def)
            .expect("definition should be entered by EnterSymbols");
        a.nested_scope.push(ScopeOwner::Symbol(symbol));
        let res = self.super_visit(a, node);
        a.nested_scope.pop();
        res
    }
}

impl<'ast> Visitor<'ast> for CheckUses<'ast> {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    fn visit_def_component(
        &self,
        a: &mut Analysis,
        node: &'ast DefComponent,
    ) -> ControlFlow<Self::Break> {
        self.visit_in_scope(a, node, Node::DefComponent(node))
    }

    /// Resolve the representation type in the enclosing scope, and the
    /// constants and the default value in the scope of the enum
    fn visit_def_enum(&self, a: &mut Analysis, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        node.type_name.visit(a, self)?;
        let symbol = a.get_symbol(node);
        if let Some(symbol) = &symbol {
            a.nested_scope.push(ScopeOwner::Symbol(symbol.clone()));
        }
        let res = match node.constants.visit(a, self) {
            ControlFlow::Continue(()) => node.default.visit(a, self),
            res => res,
        };
        if symbol.is_some() {
            a.nested_scope.pop();
        }
        res
    }

    fn visit_def_module(
        &self,
        a: &mut Analysis,
        node: &'ast DefModule,
    ) -> ControlFlow<Self::Break> {
        self.visit_in_scope(a, node, Node::DefModule(node))
    }

    /// Resolve the names inside an expanded template.
    ///
    /// We do use-analysis on the scope of the definition + param scope. A name
    /// in the body of a template resolves at the point of the template
    /// definition, not at the point of expansion, so the nested scope used here
    /// is built from scratch: the nested scope at the point of the definition,
    /// with the bound parameters and the definitions of the expansion pushed on
    /// top.
    fn visit_spec_template_expand(
        &self,
        a: &mut Analysis,
        node: &'ast SpecTemplateExpand,
    ) -> ControlFlow<Self::Break> {
        let Some(members) = &node.members else {
            // The template of an expansion that is not expanded did not
            // resolve, and CheckTemplateUses reported that
            return ControlFlow::Continue(());
        };
        if !a.template_expansion_map.contains_key(&node.node_id) {
            return ControlFlow::Continue(());
        }
        let Ok(Some(tmpl)) = a.get_template_symbol(node.template.id()) else {
            return ControlFlow::Continue(());
        };

        let mut nested_scope = a.nested_scope_at(&tmpl);
        nested_scope.push(ScopeOwner::TemplateParams(node.node_id));
        nested_scope.push(ScopeOwner::TemplateExpansion(node.node_id));

        // Analyze the inside of the template expansion
        let saved_scope = std::mem::replace(&mut a.nested_scope, nested_scope);
        let saved_expansion = a.template_expansion.replace(node.node_id);
        let res = members.visit(a, self);

        // Reset the use analysis scope back to the outer scope
        a.nested_scope = saved_scope;
        a.template_expansion = saved_expansion;
        res?;

        // Analyze the parameter list
        let params = a.expansion_at(node.node_id).params.clone();
        for param in &params {
            self.template_param(a, param)?;
        }
        ControlFlow::Continue(())
    }

    fn visit_def_state_machine(
        &self,
        a: &mut Analysis,
        node: &'ast DefStateMachine,
    ) -> ControlFlow<Self::Break> {
        self.visit_in_scope(a, node, Node::DefStateMachine(node))
    }
}

impl<'ast> UseAnalysisPass<'ast, Analysis> for CheckUses<'ast> {
    fn component_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        check_qual_ident(a, NameGroup::Component, node);
        ControlFlow::Continue(())
    }

    fn state_machine_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        check_qual_ident(a, NameGroup::StateMachine, node);
        ControlFlow::Continue(())
    }

    fn constant_use(
        &self,
        a: &mut Analysis,
        node: &Expr,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        match &node.kind {
            ExprKind::Ident { value, is_absolute } => {
                self.visit_expr_ident(a, node, value, *is_absolute);
                ControlFlow::Continue(())
            }
            ExprKind::Dot { e, id } => self.visit_expr_dot(a, node, e, id),
            _ => panic!("constant use should be qualified identifier"),
        }
    }

    // Check that we found the required implied use, and not some other symbol
    fn implied_use(
        &self,
        a: &mut Analysis,
        iu: &ImpliedUse,
        _: ImpliedUseKind,
    ) -> ControlFlow<Self::Break> {
        // A use that does not resolve has been reported already
        let Some(sym) = a.use_def_map.get(&iu.id()).cloned() else {
            return ControlFlow::Continue(());
        };
        let iu_name = iu.name().to_string();
        if sym.is_template_arg() {
            // Definition is a template argument
            // This should not happen, because implied uses are absolute
            // qualified identifiers
            panic!("definition should not be a template arg");
        }
        let sym_qualified_name = a.get_qualified_name(&sym);
        // Check that the name of the def matches the name of the use
        if sym_qualified_name != iu_name {
            if sym_qualified_name.len() > iu_name.len() {
                // Definition has a longer name: it shadows the required definition
                // This should not happen, because implied uses are absolute
                // qualified identifiers
                panic!("definition should not have a longer name");
            }
            // Definition has a shorter name: the use is a member of the definition
            SemanticError::InvalidSymbol {
                name: sym_qualified_name,
                loc: iu.id().span(),
                msg: format!("it has {} as a member", iu_name),
                def_loc: sym.get_loc(),
            }
            .emit();
        }
        ControlFlow::Continue(())
    }

    fn port_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        check_qual_ident(a, NameGroup::Port, node);
        ControlFlow::Continue(())
    }

    fn interface_instance_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        check_qual_ident(a, NameGroup::PortInterfaceInstance, node);
        ControlFlow::Continue(())
    }

    fn interface_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        check_qual_ident(a, NameGroup::PortInterface, node);
        ControlFlow::Continue(())
    }

    fn template_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        check_qual_ident(a, NameGroup::Template, node);
        ControlFlow::Continue(())
    }

    fn type_use(
        &self,
        a: &mut Analysis,
        node: &TypeName,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        match &node.kind {
            TypeNameKind::QualIdent(qual_ident) => {
                check_qual_ident(a, NameGroup::Type, qual_ident);
                if let Some(symbol) = a.use_def_map.get(&qual_ident.id()).cloned() {
                    a.use_def_map.insert(node.node_id, symbol);
                }
            }
            _ => panic!("type use should be qualified identifier"),
        }
        ControlFlow::Continue(())
    }

    fn template_constant_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        visit_template_arg(self, a, arg)
    }

    fn template_type_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        visit_template_arg(self, a, arg)
    }

    fn template_interface_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        visit_template_arg(self, a, arg)
    }
}
