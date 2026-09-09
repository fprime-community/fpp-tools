use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{BasicUseAnalyzer, UseAnalysisPass};
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{NameGroup, QualifiedName, Symbol, SymbolInterface};
use fpp_ast::{AstNode, Expr, ExprKind, Ident, Node, QualIdent, Visitable, Visitor};
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

impl<'ast> CheckUses<'ast> {
    pub fn new() -> CheckUses<'ast> {
        Self {
            super_: BasicUseAnalyzer::new(),
        }
    }

    fn visit_qualified(
        &self,
        a: &mut Analysis,
        ng: NameGroup,
        qualifier: &QualIdent,
        name: &Ident,
    ) -> SemanticResult<Symbol> {
        let qual_sym = self.visit_qual_ident_impl(a, ng, qualifier)?;
        let scope = {
            match a.symbol_scope_map.get(&qual_sym) {
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

    /// Visit a qualified identifier node and check a use
    fn visit_qual_ident_impl(
        &self,
        a: &mut Analysis,
        ng: NameGroup,
        node: &QualIdent,
    ) -> SemanticResult<Symbol> {
        match &node {
            QualIdent::Unqualified(name) => match a.symbol_get(ng, &name.data) {
                None => Err(SemanticError::UndefinedSymbol {
                    ng: ng.to_string(),
                    name: name.data.clone(),
                    loc: name.span(),
                }),
                Some(sym) => {
                    a.use_def_map.insert(name.id(), sym.clone());
                    a.use_def_map.insert(node.id(), sym.clone());
                    Ok(sym)
                }
            },
            QualIdent::Qualified(qualified) => {
                self.visit_qualified(a, ng, qualified.qualifier.deref(), &qualified.name)
            }
        }
    }

    fn visit_qual_ident(&self, a: &mut Analysis, ng: NameGroup, node: &QualIdent) {
        match self.visit_qual_ident_impl(a, ng, node) {
            Ok(sym) => {
                a.use_def_map.insert(node.id(), sym);
            }
            Err(err) => {
                err.emit();
            }
        }
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
                msg: format!("invalid use of symbol {}: {}", sym_qualified_name, msg),
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
                    Some(Symbol::Constant(_)) => None,

                    // The left side is some symbol other than a constant (a qualifier),
                    // look up this symbol and add it to the use-def entries
                    Some(qual) => {
                        let scope = a.symbol_scope_map.get(qual).unwrap();
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
            ExprKind::Ident(id) => match a.symbol_get(NameGroup::Value, id) {
                None => {
                    SemanticError::UndefinedSymbol {
                        ng: NameGroup::Value.to_string(),
                        name: id.clone(),
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
            ExprKind::Ident(id) => id,
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
}
