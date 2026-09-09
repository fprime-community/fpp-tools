use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::UseAnalysisPass;
use crate::analyzers::use_analyzer::UseAnalyzer;
use crate::errors::SemanticError;
use crate::semantics::{
    AbsType, AliasType, AnonArrayType, AnonStructType, ArrayType, EnumType, QualifiedName,
    StructType, Symbol, SymbolInterface, Type,
};
use fpp_ast::*;
use fpp_core::Spanned;
use rustc_hash::FxHashMap as HashMap;
use std::ops::{ControlFlow, Deref};
use std::sync::Arc;

/// Compute and check the types of type definition symbols, enumerated
/// constant symbols, and type names, except that array size expressions and
/// default value expressions are still unevaluated.
pub struct CheckTypeUses<'ast> {
    super_: UseAnalyzer<'ast, Self>,
}

impl<'ast> Default for CheckTypeUses<'ast> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> CheckTypeUses<'ast> {
    pub fn new() -> CheckTypeUses<'ast> {
        Self {
            super_: UseAnalyzer::new(),
        }
    }
}

impl<'ast> Visitor<'ast> for CheckTypeUses<'ast> {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    fn visit_def_abs_type(
        &self,
        a: &mut Self::State,
        node: &'ast DefAbsType,
    ) -> ControlFlow<Self::Break> {
        if a.type_map.contains_key(&node.id()) {
            return ControlFlow::Continue(());
        }

        a.type_map.insert(
            node.node_id,
            Arc::new(Type::AbsType(AbsType {
                node: node.clone(),
                default_value: None,
            })),
        );
        ControlFlow::Continue(())
    }

    fn visit_def_alias_type(
        &self,
        a: &mut Self::State,
        node: &'ast DefAliasType,
    ) -> ControlFlow<Self::Break> {
        if a.type_map.contains_key(&node.id()) {
            return ControlFlow::Continue(());
        }

        // Make sure the type uses are mapped
        node.walk(a, self)?;

        let alias_type = a.type_map.get(&node.type_name.node_id).unwrap().clone();
        a.type_map.insert(
            node.node_id,
            Arc::new(Type::AliasType(AliasType {
                node: node.clone(),
                alias_type,
            })),
        );

        ControlFlow::Continue(())
    }

    fn visit_def_array(
        &self,
        a: &mut Self::State,
        node: &'ast DefArray,
    ) -> ControlFlow<Self::Break> {
        if a.type_map.contains_key(&node.id()) {
            return ControlFlow::Continue(());
        }

        node.walk(a, self)?;
        let elt_type = a.type_map.get(&node.elt_type.node_id).unwrap().clone();

        a.type_map.insert(
            node.node_id,
            Arc::new(Type::Array(ArrayType {
                node: node.clone(),
                anon_array: AnonArrayType {
                    size: None,
                    elt_type,
                },
                default: None,
                format: None,
            })),
        );

        ControlFlow::Continue(())
    }

    fn visit_def_enum(&self, a: &mut Self::State, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        if a.type_map.contains_key(&node.id()) {
            return ControlFlow::Continue(());
        }

        node.walk(a, self)?;
        if node.constants.is_empty() {
            SemanticError::InvalidType {
                loc: node.span(),
                msg: "enum must define at least one constant".to_string(),
            }
            .emit();
        }

        let rep_type = {
            match &node.type_name {
                None => IntegerKind::I32,
                Some(type_name) => {
                    let ty = a.type_map.get(&type_name.node_id).unwrap();
                    match Type::underlying_type(ty).deref() {
                        Type::PrimitiveInt(kind) => *kind,
                        _ => {
                            SemanticError::InvalidType {
                                loc: type_name.span(),
                                msg: "primitive integer type must be used".to_string(),
                            }
                            .emit();
                            IntegerKind::I32
                        }
                    }
                }
            }
        };

        let ty = Arc::new(Type::Enum(EnumType {
            node: node.clone(),
            rep_type,
            default: None,
        }));

        a.type_map.insert(node.node_id, ty.clone());

        // Assign types to the constant members
        for member in &node.constants {
            a.type_map.insert(member.node_id, ty.clone());
        }

        ControlFlow::Continue(())
    }

    fn visit_def_struct(
        &self,
        a: &mut Self::State,
        node: &'ast DefStruct,
    ) -> ControlFlow<Self::Break> {
        if a.type_map.contains_key(&node.id()) {
            return ControlFlow::Continue(());
        }

        // Visit all the members to resolve type info
        node.walk(a, self)?;

        let mut member_locs = HashMap::default();
        let mut anon_ty = AnonStructType {
            members: Default::default(),
        };

        for member in &node.members {
            match member_locs.insert(member.name.data.clone(), member.span()) {
                None => {
                    let member_ty = a.type_map.get(&member.type_name.node_id).unwrap().clone();
                    anon_ty.members.insert(member.name.data.clone(), member_ty);
                }
                Some(old) => {
                    SemanticError::DuplicateStructMember {
                        name: member.name.data.clone(),
                        loc: member.span(),
                        prev_loc: old,
                    }
                    .emit();
                }
            }
        }

        a.type_map.insert(
            node.node_id,
            Arc::new(Type::Struct(StructType {
                node: node.clone(),
                anon_struct: anon_ty,
                default: None,
                sizes: Default::default(),
                formats: Default::default(),
            })),
        );

        ControlFlow::Continue(())
    }

    fn visit_expr(&self, a: &mut Self::State, node: &'ast Expr) -> ControlFlow<Self::Break> {
        // Expressions are not type uses, so we do not delegate to the use
        // analyzer chain. We still walk sub-expressions so that a `sizeof(T)`
        // nested anywhere in an expression has its type name resolved by
        // `visit_type_name`.
        node.walk(a, self)
    }

    fn visit_type_name(
        &self,
        a: &mut Self::State,
        node: &'ast TypeName,
    ) -> ControlFlow<Self::Break> {
        let ty = match &node.kind {
            TypeNameKind::Bool => Type::Boolean,
            TypeNameKind::Floating(kind) => Type::Float(*kind),
            TypeNameKind::Integer(kind) => Type::PrimitiveInt(*kind),
            TypeNameKind::QualIdent(qi) => {
                self.super_visit(a, Node::TypeName(node))?;
                let ty = match a.type_map.get(&qi.id()) {
                    Some(qi_ty) => qi_ty.clone(),
                    // The type use did not resolve. Map it to the shared
                    // unknown type so every type name has an entry and later
                    // passes can read a resolved type unconditionally.
                    None => a.unknown_type(node.span()),
                };
                a.type_map.insert(node.node_id, ty);
                return ControlFlow::Continue(());
            }
            TypeNameKind::String(_) => Type::String(None),
        };

        a.type_map.insert(node.node_id, Arc::new(ty));
        ControlFlow::Continue(())
    }
}

impl<'ast> UseAnalysisPass<'ast, Analysis> for CheckTypeUses<'ast> {
    fn type_use(
        &self,
        a: &mut Analysis,
        node: &QualIdent,
        _name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let symbol = match a.use_def_map.get(&node.id()) {
            // Symbol reference does not exist, give up
            None => return ControlFlow::Continue(()),
            Some(symbol) => symbol.clone(),
        };

        match &symbol {
            Symbol::AbsType(def) => def.visit(a, self)?,
            Symbol::AliasType(def) => def.visit(a, self)?,
            Symbol::Array(def) => def.visit(a, self)?,
            Symbol::Enum(def) => def.visit(a, self)?,
            Symbol::Struct(def) => def.visit(a, self)?,
            _ => {
                SemanticError::InvalidSymbol {
                    symbol_name: symbol.name().data.clone(),
                    msg: "not a type symbol".to_string(),
                    loc: node.span(),
                    def_loc: symbol.name().span(),
                }
                .emit();
                return ControlFlow::Continue(());
            }
        };

        match a.type_map.get(&symbol.node()) {
            None => {}
            Some(ty) => {
                a.type_map.insert(node.id(), ty.clone());
            }
        }

        ControlFlow::Continue(())
    }
}
