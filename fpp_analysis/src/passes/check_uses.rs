use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{BasicUseAnalyzer, UseAnalysisPass, visit_template_arg};
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{NameGroup, QualifiedName, ScopeOwner, Symbol, SymbolInterface};
use fpp_ast::{
    AstNode, Expr, ExprKind, Ident, Node, QualIdent, SpecTemplateExpand, Visitable, Visitor,
};
use fpp_core::Spanned;
use std::ops::{ControlFlow, Deref};

/// Match uses to their definitions
pub struct CheckUses<'ast> {
    super_: BasicUseAnalyzer<'ast, Analysis, Self>,
}

impl<'ast> Default for CheckUses<'ast> {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve `qualifier` `.` `name` in name group `ng`, recording the use
fn resolve_qualified(
    a: &mut Analysis,
    ng: NameGroup,
    qualifier: &QualIdent,
    name: &Ident,
) -> SemanticResult<Symbol> {
    let qual_sym = resolve_qual_ident(a, ng, qualifier)?;
    let scope = {
        match a.lookup_scope(&qual_sym) {
            None => {
                return Err(SemanticError::InvalidSymbol {
                    symbol_name: qual_sym.name().data.clone(),
                    msg: "not a qualifier".to_string(),
                    loc: qualifier.span(),
                    def_loc: qual_sym.node().span(),
                });
            }
            Some(scope) => scope,
        }
    };

    match scope.get(ng, &name.data) {
        None => Err(SemanticError::UndefinedSymbol {
            ng: ng.to_string(),
            name: name.data.clone(),
            loc: name.span(),
        }),
        Some(sym) => {
            a.use_def_map.insert(name.id(), sym.clone());
            Ok(sym)
        }
    }
}

/// Resolve a qualified identifier node in name group `ng`, recording the use
pub(crate) fn resolve_qual_ident(
    a: &mut Analysis,
    ng: NameGroup,
    node: &QualIdent,
) -> SemanticResult<Symbol> {
    match &node {
        QualIdent::Unqualified(unqualified) => match a.symbol_get(ng, &unqualified.name.data) {
            None => Err(SemanticError::UndefinedSymbol {
                ng: ng.to_string(),
                name: unqualified.name.data.clone(),
                loc: unqualified.name.span(),
            }),
            Some(sym) => {
                a.use_def_map.insert(unqualified.name.id(), sym.clone());
                a.use_def_map.insert(node.id(), sym.clone());
                Ok(sym)
            }
        },
        QualIdent::Qualified(qualified) => {
            resolve_qualified(a, ng, qualified.qualifier.deref(), &qualified.name)
        }
    }
}

/// Resolve a qualified identifier node in name group `ng`, recording the use and
/// emitting any error
pub(crate) fn check_qual_ident(a: &mut Analysis, ng: NameGroup, node: &QualIdent) {
    match resolve_qual_ident(a, ng, node) {
        Ok(sym) => {
            a.use_def_map.insert(node.id(), sym);
        }
        Err(err) => {
            err.emit();
        }
    }
}

impl<'ast> CheckUses<'ast> {
    pub fn new() -> CheckUses<'ast> {
        Self {
            super_: BasicUseAnalyzer::new(),
        }
    }

    fn visit_qual_ident_impl(
        &self,
        a: &mut Analysis,
        ng: NameGroup,
        node: &QualIdent,
    ) -> SemanticResult<Symbol> {
        resolve_qual_ident(a, ng, node)
    }

    fn visit_qual_ident(&self, a: &mut Analysis, ng: NameGroup, node: &QualIdent) {
        check_qual_ident(a, ng, node)
    }

    // Check that an implied use (a) is not a member of a def and (b) does not
    // shadow the required def. Same check as in `implied_port_use`.
    fn implied_use_shadow_check(
        &self,
        a: &Analysis,
        loc: fpp_core::Span,
        iu_name: &QualifiedName,
        sym: &Symbol,
    ) {
        // A bound template parameter is not a global definition, so it cannot
        // satisfy an implied use
        if sym.is_template_arg() {
            SemanticError::InvalidSymbol {
                symbol_name: sym.name().data.clone(),
                msg: format!(
                    "a template parameter may not satisfy the implied use of {}",
                    iu_name
                ),
                loc,
                def_loc: sym.node().span(),
            }
            .emit();
            return;
        }
        let sym_qualified_name = a.get_qualified_name(sym);
        let iu_name = iu_name.to_string();
        // Check that the name of the def matches the name of the use
        if sym_qualified_name != iu_name {
            let msg = if sym_qualified_name.len() < iu_name.len() {
                // Definition has a shorter name: the use is a member of the definition
                format!("it has {} as a member", iu_name)
            } else {
                // Definition has a longer name: it shadows the required definition
                format!("it shadows {} here", iu_name)
            };
            SemanticError::InvalidSymbol {
                symbol_name: sym_qualified_name.clone(),
                msg,
                loc,
                def_loc: sym.node().span(),
            }
            .emit();
        }
    }
}

impl<'ast> Visitor<'ast> for CheckUses<'ast> {
    type Break = ();
    type State = Analysis;

    // Walk all nodes deeply and collect up scope where relevant
    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    /// Resolve the names inside an expanded template.
    ///
    /// A name in the body of a template resolves at the point of the template
    /// definition, not at the point of expansion. So the scope stack used here is
    /// built from scratch: the stack at the point of the definition, with the
    /// bound parameters and the definitions of the expansion pushed on top.
    fn visit_spec_template_expand(
        &self,
        a: &mut Analysis,
        node: &'ast SpecTemplateExpand,
    ) -> ControlFlow<Self::Break> {
        let Some(members) = &node.members else {
            // This specifier was not expanded: CheckTemplateUses has already
            // reported why
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

        let saved_scope = std::mem::replace(&mut a.nested_scope, nested_scope);
        let saved_expansion = a.template_expansion.replace(node.node_id);
        let res = members.visit(a, self);
        a.nested_scope = saved_scope;
        a.template_expansion = saved_expansion;
        res?;

        // Analyze the argument list, which is written at the expansion site and
        // so resolves in the enclosing scope
        let params: Vec<Symbol> = a
            .expansion_at(node.node_id)
            .params
            .values()
            .cloned()
            .collect();
        for param in &params {
            self.template_param(a, param)?;
        }
        ControlFlow::Continue(())
    }
}

impl<'ast> UseAnalysisPass<'ast, Analysis> for CheckUses<'ast> {
    fn component_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = name;
        self.visit_qual_ident(a, NameGroup::Component, node);
        ControlFlow::Continue(())
    }

    fn interface_instance_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = name;
        self.visit_qual_ident(a, NameGroup::PortInterfaceInstance, node);
        ControlFlow::Continue(())
    }

    fn constant_use(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        match &node.kind {
            ExprKind::Dot { e, id } => {
                // Visit the left side of the dot recursively
                e.visit(a, self)?;

                // Find the symbol referred to by the left side (if-any)
                let symbol = match a.use_def_map.get(&e.id()) {
                    // Left-hand side is not a symbol
                    // There is no resolution on the dot expression
                    None => None,
                    // Left side is a constant, we are selecting a member of this constant
                    // we are not creating another use.
                    Some(Symbol::Constant(_) | Symbol::TemplateConstantArg { .. }) => None,

                    // The left side is some symbol other than a constant (a qualifier),
                    // look up this symbol and add it to the use-def entries
                    Some(qual) => {
                        let scope = a.lookup_scope(qual).expect("qualifier has a scope");
                        match scope.get(NameGroup::Value, &id.data) {
                            None => {
                                SemanticError::UndefinedSymbol {
                                    ng: NameGroup::Value.to_string(),
                                    name: id.data.clone(),
                                    loc: id.span(),
                                }
                                .emit();
                                None
                            }
                            Some(sym) => Some(sym),
                        }
                    }
                };

                match symbol {
                    None => {}
                    Some(symbol) => {
                        a.use_def_map.insert(node.id(), symbol);
                    }
                }
            }
            ExprKind::Ident { value, .. } => match a.symbol_get(NameGroup::Value, value) {
                None => {
                    SemanticError::UndefinedSymbol {
                        ng: NameGroup::Value.to_string(),
                        name: value.clone(),
                        loc: node.span(),
                    }
                    .emit();
                }
                Some(symbol) => {
                    a.use_def_map.insert(node.id(), symbol);
                }
            },
            _ => panic!("constant use should be qualified identifier"),
        }

        ControlFlow::Continue(())
    }

    fn port_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = name;
        self.visit_qual_ident(a, NameGroup::Port, node);
        ControlFlow::Continue(())
    }

    // Check that an implied use (a) is not a member
    // of a def and (b) does not shadow the required def
    fn implied_port_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let sym = match self.visit_qual_ident_impl(a, NameGroup::Port, node) {
            Ok(sym) => sym,
            Err(err) => {
                err.emit();
                return ControlFlow::Continue(());
            }
        };
        a.use_def_map.insert(node.id(), sym.clone());
        self.implied_use_shadow_check(a, node.span(), &name, &sym);
        ControlFlow::Continue(())
    }

    // An implied use of a type (e.g. `FwSizeStoreType` for a string type).
    // Resolve it and check it does not shadow / is not a member of another def.
    fn implied_type_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let sym = match self.visit_qual_ident_impl(a, NameGroup::Type, node) {
            Ok(sym) => sym,
            Err(err) => {
                err.emit();
                return ControlFlow::Continue(());
            }
        };
        a.use_def_map.insert(node.id(), sym.clone());
        self.implied_use_shadow_check(a, node.span(), &name, &sym);
        ControlFlow::Continue(())
    }

    // An implied use of a constant (e.g. `FW_FIXED_LENGTH_STRING_SIZE` for a
    // default-size string type). Implied constant uses are always unqualified.
    fn implied_constant_use(
        &self,
        a: &mut Analysis,
        node: &Expr,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let ident = match &node.kind {
            ExprKind::Ident { value, .. } => value,
            _ => return ControlFlow::Continue(()),
        };
        let sym = match a.symbol_get(NameGroup::Value, ident) {
            None => {
                SemanticError::UndefinedSymbol {
                    ng: NameGroup::Value.to_string(),
                    name: ident.clone(),
                    loc: node.span(),
                }
                .emit();
                return ControlFlow::Continue(());
            }
            Some(sym) => sym,
        };
        a.use_def_map.insert(node.id(), sym.clone());
        self.implied_use_shadow_check(a, node.span(), &name, &sym);
        ControlFlow::Continue(())
    }

    fn interface_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = name;
        self.visit_qual_ident(a, NameGroup::PortInterface, node);
        ControlFlow::Continue(())
    }

    fn type_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = name;
        self.visit_qual_ident(a, NameGroup::Type, node);
        ControlFlow::Continue(())
    }

    fn state_machine_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = name;
        self.visit_qual_ident(a, NameGroup::StateMachine, node);
        ControlFlow::Continue(())
    }

    fn template_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = name;
        self.visit_qual_ident(a, NameGroup::Template, node);
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
