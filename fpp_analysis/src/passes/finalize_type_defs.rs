use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::nested_analyzer::{NestedAnalyzer, NestedAnalyzerMode};
use crate::errors::SemanticError;
use crate::semantics::{
    AliasType, AnonArrayType, AnonStructType, ArrayType, ArrayValue, Format, IntegerValue,
    StructType, StructValue, Symbol, SymbolInterface, Type, Value,
};
use fpp_ast::{
    AstNode, DefAliasType, DefArray, DefEnum, DefStruct, Expr, Node, TransUnit, TypeName,
    TypeNameKind, Visitor, Walkable,
};
use fpp_core::Spanned;
use std::ops::{ControlFlow, Deref};
use std::sync::Arc;

/// Finalize type definitions. Update the types of uses (type names) that
/// refer to the definitions.
pub struct FinalizeTypeDefs<'ast> {
    super_: NestedAnalyzer<'ast, Analysis, Self>,
}

impl<'ast> Default for FinalizeTypeDefs<'ast> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> FinalizeTypeDefs<'ast> {
    pub fn new() -> FinalizeTypeDefs<'ast> {
        Self {
            super_: NestedAnalyzer::new(NestedAnalyzerMode::DEEP),
        }
    }

    fn expr_as_integer(&self, a: &mut Analysis, e: &Expr) -> Option<i128> {
        match a.value_map.get(&e.node_id) {
            None => None,
            Some(v) => {
                if let Some(Value::Integer(IntegerValue(i))) = v.convert(&Arc::new(Type::Integer)) {
                    Some(i)
                } else {
                    None
                }
            }
        }
    }

    fn expr_as_integer_opt(&self, a: &mut Analysis, e: &Option<Expr>) -> Option<i128> {
        match e {
            None => None,
            Some(e) => self.expr_as_integer(a, e),
        }
    }

    /// Finalizes the type named by `node` and returns it.
    pub(crate) fn ty(&self, a: &mut Analysis, node: &'ast TypeName) -> ControlFlow<(), Arc<Type>> {
        match &node.kind {
            TypeNameKind::QualIdent(q) => match a.use_def_map.get(&q.id()).cloned() {
                None => {}
                Some(symbol) => {
                    match &symbol {
                        Symbol::AbsType(ty) => self.visit_def_abs_type(a, ty.deref()),
                        Symbol::AliasType(ty) => self.visit_def_alias_type(a, ty.deref()),
                        Symbol::ArrayType(ty) => self.visit_def_array(a, ty.deref()),
                        Symbol::EnumType(ty) => self.visit_def_enum(a, ty.deref()),
                        Symbol::StructType(ty) => self.visit_def_struct(a, ty.deref()),
                        _ => ControlFlow::Continue(()),
                    }?;

                    if let Some(def_ty) = a.type_map.get(&symbol.node()).cloned() {
                        a.type_map.insert(node.node_id, def_ty);
                    }
                }
            },
            TypeNameKind::String(size) => match self.expr_as_integer_opt(a, size) {
                None => {}
                Some(size_v) => {
                    // TODO(tumbar) Should we disallow 0 size strings?
                    //    See https://github.com/nasa/fpp/issues/878
                    if size_v < 0 {
                        SemanticError::InvalidIntValue {
                            loc: size.as_ref().unwrap().span(),
                            v: Some(size_v),
                            msg: "negative string sizes are not allowed".to_string(),
                        }
                        .emit();
                        return ControlFlow::Break(());
                    } else if size_v >= 1 << 31 {
                        SemanticError::InvalidIntValue {
                            loc: size.as_ref().unwrap().span(),
                            v: Some(size_v),
                            msg: "string size must in range [0, 2^31)".to_string(),
                        }
                        .emit();
                        return ControlFlow::Break(());
                    } else {
                        a.type_map
                            .insert(node.node_id, Arc::new(Type::String(Some(size_v))));
                    }
                }
            },
            _ => {}
        }

        // `CheckTypeUses` normally gives every type name it reaches an entry.
        // Fall back to the shared unknown type rather than assuming that: this
        // is called on demand from `EvalConstantExprs` for `sizeof` operands,
        // which can sit in positions that no earlier pass visited, and a
        // missing entry must degrade to an unknown type, not abort the
        // compiler (which would also take down `fpp_lsp_server`).
        ControlFlow::Continue(match a.type_map.get(&node.node_id).cloned() {
            Some(ty) => ty,
            None => {
                let ty = a.unknown_type(node.span());
                a.type_map.insert(node.node_id, ty.clone());
                ty
            }
        })
    }
}

impl<'ast> Visitor<'ast> for FinalizeTypeDefs<'ast> {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    // No more need to evaluate sub-expressions
    fn visit_expr(&self, _: &mut Self::State, _: &'ast Expr) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }

    fn visit_type_name(&self, a: &mut Self::State, node: &'ast TypeName) -> ControlFlow<()> {
        if matches!(node.kind, TypeNameKind::String(_)) {
            self.ty(a, node)?;
        }
        ControlFlow::Continue(())
    }

    fn visit_trans_unit(
        &self,
        a: &mut Self::State,
        node: &'ast TransUnit,
    ) -> ControlFlow<Self::Break> {
        a.visited_symbol_set.clear();
        node.walk(a, self)
    }

    fn visit_def_alias_type(
        &self,
        a: &mut Self::State,
        node: &'ast DefAliasType,
    ) -> ControlFlow<Self::Break> {
        let Some(symbol) = a.get_symbol(node) else {
            return ControlFlow::Continue(());
        };
        if a.visited_symbol_set.contains(&symbol) {
            return ControlFlow::Continue(());
        }

        let def = a.interned_def(node);
        a.visited_symbol_set.insert(symbol);
        // Finalize the referenced type
        let ty = self.ty(a, &node.type_name)?;
        // Update the alias type in the type map
        a.type_map.insert(
            node.node_id,
            Arc::new(Type::Alias(AliasType {
                node: def,
                alias_type: ty,
            })),
        );

        ControlFlow::Continue(())
    }

    fn visit_def_array(
        &self,
        a: &mut Self::State,
        node: &'ast DefArray,
    ) -> ControlFlow<Self::Break> {
        let Some(symbol) = a.get_symbol(node) else {
            return ControlFlow::Continue(());
        };
        if a.visited_symbol_set.contains(&symbol) {
            return ControlFlow::Continue(());
        }

        let def = a.interned_def(node);
        a.visited_symbol_set.insert(symbol);
        // Finalize the element type
        let elt_type = self.ty(a, &node.elt_type)?;

        let size = match self.expr_as_integer(a, &node.size) {
            None => return ControlFlow::Continue(()),
            Some(size) => size,
        };

        if size < i32::MIN as i128 || size > i32::MAX as i128 {
            SemanticError::InvalidIntValue {
                loc: node.size.span(),
                v: Some(size),
                msg: "value out of range".to_string(),
            }
            .emit();
            return ControlFlow::Break(());
        }

        if size <= 0 {
            SemanticError::InvalidIntValue {
                loc: node.size.span(),
                v: Some(size),
                msg: "array size must be greater than zero".to_string(),
            }
            .emit();
            return ControlFlow::Break(());
        }

        // Update the size and element type
        let anon_array = AnonArrayType {
            size: Some(size as usize),
            elt_type: elt_type.clone(),
        };

        let anon_array_ty = Arc::new(Type::AnonArray(anon_array.clone()));

        // The array type without its default value or format
        let mut array_ty = ArrayType {
            node: def,
            anon_array,
            default: None,
            format: None,
        };
        // The array type as it stands, for the default value to name. The
        // default is part of the type, so the value names the type without it.
        let array_ty_arc = Arc::new(array_ty.clone());
        let array_ty_a = Arc::new(Type::Array(array_ty_arc.clone()));

        // Compute the default value
        array_ty.default = match &node.default {
            None => match anon_array_ty.default_value() {
                Some(Value::AnonArray(anon_array)) => Some(ArrayValue {
                    anon_array,
                    ty: array_ty_arc.clone(),
                }),
                _ => None,
            },
            Some(default) => match a.value_map.get(&default.node_id).cloned() {
                None => None,
                Some(default_v) => {
                    if let Value::AnonArray(v) | Value::Array(ArrayValue { anon_array: v, .. }) =
                        &default_v
                        && v.elements.len() != size as usize
                    {
                        SemanticError::ArrayDefaultMismatchedSize {
                            loc: default.span(),
                            size_loc: node.size.span(),
                            value_size: v.elements.len(),
                            type_size: size,
                        }
                        .emit();
                        return ControlFlow::Break(());
                    }
                    match default_v.convert(&array_ty_a) {
                        Some(Value::Array(v)) => Some(v),
                        _ => None,
                    }
                }
            },
        };

        // Compute the format
        array_ty.format = node
            .format
            .as_ref()
            .map(|format| Format::new(format, vec![(elt_type.clone(), node.elt_type.span())]));

        // Update the array type in the type map
        a.type_map
            .insert(node.node_id, Arc::new(Type::Array(Arc::new(array_ty))));
        ControlFlow::Continue(())
    }

    fn visit_def_enum(&self, a: &mut Self::State, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        let Some(symbol) = a.get_symbol(node) else {
            return ControlFlow::Continue(());
        };
        if a.visited_symbol_set.contains(&symbol) {
            return ControlFlow::Continue(());
        }

        a.visited_symbol_set.insert(symbol);
        // `CheckTypeUses` always enters an enum type for the definition node.
        let mut enum_ty = match a.type_map.get(&node.node_id).unwrap().deref() {
            Type::Enum(ty) => ty.deref().clone(),
            _ => panic!("expected enum type"),
        };

        let default_value = match &node.default {
            None => {
                // Choose the first value
                match node.constants.first() {
                    None => None,
                    Some(first_constant) => a.value_map.get(&first_constant.node_id).cloned(),
                }
            }
            Some(def) => a.value_map.get(&def.node_id).cloned(),
        };
        enum_ty.default = match default_value {
            Some(Value::EnumConstant(v)) => Some(v),
            _ => None,
        };

        a.type_map
            .insert(node.node_id, Arc::new(Type::Enum(Arc::new(enum_ty))));

        ControlFlow::Continue(())
    }

    fn visit_def_struct(
        &self,
        a: &mut Self::State,
        node: &'ast DefStruct,
    ) -> ControlFlow<Self::Break> {
        let Some(symbol) = a.get_symbol(node) else {
            return ControlFlow::Continue(());
        };
        if a.visited_symbol_set.contains(&symbol) {
            return ControlFlow::Continue(());
        }

        let def = a.interned_def(node);
        a.visited_symbol_set.insert(symbol);

        let mut ty = StructType {
            node: def,
            anon_struct: AnonStructType {
                members: Default::default(),
            },
            default: None,
            sizes: Default::default(),
            formats: Default::default(),
        };

        for member in &node.members {
            // Finalize the member's type
            let member_ty = self.ty(a, &member.type_name)?;
            ty.anon_struct
                .members
                .push((member.name.data.clone(), member_ty.clone()));

            // Compute the size
            let size = self.expr_as_integer_opt(a, &member.size);
            match size {
                None => {}
                Some(size) if size >= 1 => {
                    if size < 1 << 31 {
                        ty.sizes.insert(member.name.data.clone(), size as u32);
                    } else {
                        SemanticError::InvalidIntValue {
                            loc: member.size.clone().unwrap().span(),
                            v: Some(size),
                            msg: "array size must be less than 2^31".to_string(),
                        }
                        .emit();
                        return ControlFlow::Break(());
                    }
                }
                Some(size) => {
                    SemanticError::InvalidIntValue {
                        loc: member.size.clone().unwrap().span(),
                        v: Some(size),
                        msg: "array size must be greater than zero".to_string(),
                    }
                    .emit();
                    return ControlFlow::Break(());
                }
            }

            // Compute the format
            if let Some(format) = &member.format {
                ty.formats.insert(
                    member.name.data.clone(),
                    Format::new(format, vec![(member_ty, member.type_name.span())]),
                );
            }
        }

        // Compute the default value. The value refers to the struct type
        // without its own default.
        let struct_ty_arc = Arc::new(ty.clone());
        let struct_ty_a = Arc::new(Type::Struct(struct_ty_arc.clone()));
        ty.default = match &node.default {
            // The convertibility of an explicit default is checked by
            // `CheckExprTypes`; store the converted value.
            Some(default) => match a.value_map.get(&default.node_id).cloned() {
                Some(default_v) => match default_v.convert(&struct_ty_a) {
                    Some(Value::Struct(v)) => Some(v),
                    _ => None,
                },
                None => None,
            },
            None => match Arc::new(Type::AnonStruct(ty.anon_struct.clone())).default_value() {
                Some(Value::AnonStruct(anon_struct)) => Some(StructValue {
                    anon_struct,
                    ty: struct_ty_arc,
                }),
                _ => None,
            },
        };

        // Update the struct type in the type map
        a.type_map
            .insert(node.node_id, Arc::new(Type::Struct(Arc::new(ty))));
        ControlFlow::Continue(())
    }
}
