use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{UseAnalysisPass, visit_template_arg};
use crate::analyzers::use_analyzer::UseAnalyzer;
use crate::errors::SemanticError;
use crate::semantics::{
    AbsType, AliasType, AnonArrayType, AnonStructType, ArrayType, EnumType, QualifiedName,
    StructType, Symbol, SymbolInterface, Type,
};
use fpp_ast::*;
use fpp_core::Spanned;
use rustc_hash::FxHashSet as HashSet;
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

    /// Visit the definition a type use refers to, and give the type name the
    /// type of the definition.
    ///
    /// A symbol that is not a type definition or a type parameter is rejected
    /// before its node is looked up in the type map. A state machine is in the
    /// type name group, and the implied use of its state enum gives its node
    /// a type.
    fn visit_use(&self, a: &mut Analysis, node: &TypeName) -> ControlFlow<()> {
        let symbol = match a.use_def_map.get(&node.node_id) {
            // The use did not resolve, which CheckUses reported
            None => return ControlFlow::Continue(()),
            Some(symbol) => symbol.clone(),
        };

        match &symbol {
            Symbol::AbsType(def) => def.visit(a, self)?,
            Symbol::AliasType(def) => def.visit(a, self)?,
            Symbol::ArrayType(def) => def.visit(a, self)?,
            Symbol::EnumType(def) => def.visit(a, self)?,
            Symbol::StructType(def) => def.visit(a, self)?,
            // A bound type parameter names the type its argument names, so
            // compute the type of the argument
            Symbol::TemplateTypeArg { value, .. } => {
                if !a.type_map.contains_key(&value.node_id) {
                    self.template_type_arg(a, &symbol)?;
                }
            }
            _ => {
                SemanticError::InvalidSymbol {
                    symbol_name: symbol.unqualified_name().to_string(),
                    msg: "not a type symbol".to_string(),
                    loc: node.span(),
                    def_loc: symbol.get_loc(),
                }
                .emit();
                return ControlFlow::Continue(());
            }
        };

        if let Some(ty) = a.type_map.get(&symbol.node()).cloned() {
            a.assign_type(node.node_id, ty);
        }

        ControlFlow::Continue(())
    }

    /// Visit a definition with `visitor`, unless the definition has a type
    fn visit_if_needed(
        &self,
        a: &mut Analysis,
        node: fpp_core::Node,
        visitor: impl FnOnce(&mut Analysis) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        if !a.type_map.contains_key(&node) {
            visitor(a)
        } else {
            ControlFlow::Continue(())
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
        self.visit_if_needed(a, node.node_id, |a| {
            let t = Arc::new(Type::Abs(Arc::new(AbsType {
                node: a.interned_def(node),
            })));
            a.assign_type(node.node_id, t);
            ControlFlow::Continue(())
        })
    }

    fn visit_def_alias_type(
        &self,
        a: &mut Self::State,
        node: &'ast DefAliasType,
    ) -> ControlFlow<Self::Break> {
        self.visit_if_needed(a, node.node_id, |a| {
            node.walk(a, self)?;
            let aliased_type = a.type_map.get(&node.type_name.node_id).unwrap().clone();
            let t = Arc::new(Type::Alias(AliasType {
                node: a.interned_def(node),
                alias_type: aliased_type,
            }));
            a.assign_type(node.node_id, t);
            ControlFlow::Continue(())
        })
    }

    fn visit_def_array(
        &self,
        a: &mut Self::State,
        node: &'ast DefArray,
    ) -> ControlFlow<Self::Break> {
        self.visit_if_needed(a, node.node_id, |a| {
            node.walk(a, self)?;
            let elt_type = a.type_map.get(&node.elt_type.node_id).unwrap().clone();
            let anon_array = AnonArrayType {
                size: None,
                elt_type,
            };
            let t = Arc::new(Type::Array(Arc::new(ArrayType {
                node: a.interned_def(node),
                anon_array,
                default: None,
                format: None,
            })));
            a.assign_type(node.node_id, t);
            ControlFlow::Continue(())
        })
    }

    fn visit_def_enum(&self, a: &mut Self::State, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        self.visit_if_needed(a, node.node_id, |a| {
            node.walk(a, self)?;
            if node.constants.is_empty() {
                SemanticError::InvalidType {
                    loc: node.span(),
                    msg: "enum must define at least one constant".to_string(),
                    notes: vec![],
                }
                .emit();
            }

            let rep_type = match &node.type_name {
                Some(type_name) => {
                    let rep_type = a.type_map.get(&type_name.node_id).unwrap();
                    match Type::underlying_type(rep_type).deref() {
                        Type::PrimitiveInt(kind) => *kind,
                        _ => {
                            SemanticError::InvalidType {
                                loc: type_name.span(),
                                msg: "primitive integer type must be used".to_string(),
                                notes: vec![],
                            }
                            .emit();
                            IntegerKind::I32
                        }
                    }
                }
                None => IntegerKind::I32,
            };

            let t = Arc::new(Type::Enum(Arc::new(EnumType {
                node: a.interned_def(node),
                rep_type,
                default: None,
            })));
            a.assign_type(node.node_id, t.clone());
            for constant in &node.constants {
                a.assign_type(constant.node_id, t.clone());
            }
            ControlFlow::Continue(())
        })
    }

    fn visit_def_struct(
        &self,
        a: &mut Self::State,
        node: &'ast DefStruct,
    ) -> ControlFlow<Self::Break> {
        self.visit_if_needed(a, node.node_id, |a| {
            // The member names are checked before the member types, which may
            // reach definitions with errors of their own
            fn get_name(member: &StructTypeMember) -> &str {
                &member.name.data
            }
            if let Err(err) = Analysis::check_for_duplicate_struct_member(get_name, &node.members) {
                err.emit();
            }
            node.walk(a, self)?;

            let mut member_names = HashSet::default();
            let mut anon_struct = AnonStructType {
                members: Default::default(),
            };
            for member in &node.members {
                // A duplicate member has been reported, and the first member of
                // the name gives the type
                if member_names.insert(member.name.data.as_str()) {
                    let t = a.type_map.get(&member.type_name.node_id).unwrap().clone();
                    anon_struct.members.push((member.name.data.clone(), t));
                }
            }
            let t = Arc::new(Type::Struct(Arc::new(StructType {
                node: a.interned_def(node),
                anon_struct,
                default: None,
                sizes: Default::default(),
                formats: Default::default(),
            })));
            a.assign_type(node.node_id, t);
            ControlFlow::Continue(())
        })
    }

    fn visit_type_name(
        &self,
        a: &mut Self::State,
        node: &'ast TypeName,
    ) -> ControlFlow<Self::Break> {
        let t = match &node.kind {
            TypeNameKind::Bool => Type::Boolean,
            TypeNameKind::Floating(kind) => Type::Float(*kind),
            TypeNameKind::Integer(kind) => Type::PrimitiveInt(*kind),
            TypeNameKind::QualIdent(_) => {
                self.super_visit(a, Node::TypeName(node))?;
                // The type use did not resolve. Map it to the shared unknown
                // type so every type name has an entry and later passes can
                // read a resolved type unconditionally.
                if !a.type_map.contains_key(&node.node_id) {
                    let t = a.unknown_type(node.span());
                    a.assign_type(node.node_id, t);
                }
                return ControlFlow::Continue(());
            }
            TypeNameKind::String(_) => {
                self.super_visit(a, Node::TypeName(node))?;
                Type::String(None)
            }
        };

        a.assign_type(node.node_id, Arc::new(t));
        ControlFlow::Continue(())
    }
}

impl<'ast> UseAnalysisPass<'ast, Analysis> for CheckTypeUses<'ast> {
    fn type_use(
        &self,
        a: &mut Analysis,
        node: &TypeName,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.visit_use(a, node)
    }

    fn template_constant_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        visit_template_arg(self, a, arg)
    }

    fn template_type_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        visit_template_arg(self, a, arg)
    }
}
