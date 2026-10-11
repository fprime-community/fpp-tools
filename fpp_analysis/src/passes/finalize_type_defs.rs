use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::type_expression_analyzer::TypeExpressionAnalyzer;
use crate::errors::SemanticError;
use crate::semantics::{
    AliasType, AnonArrayType, AnonStructType, ArrayType, ArrayValue, Format, StructType,
    StructValue, Symbol, Type, Value,
};
use fpp_ast::{
    AstNode, DefAliasType, DefArray, DefEnum, DefModuleTemplate, DefStruct, Expr, LitString, Node,
    TemplateParam, TypeName, TypeNameKind, Visitor,
};
use fpp_core::Spanned;
use std::ops::{ControlFlow, Deref};
use std::sync::Arc;

/// Finalize type definitions. Update the types of uses (type names) that
/// refer to the definitions.
///
/// The visited symbol set holds the definitions finalized so far, including
/// the ones that `EvalConstantExprs` finalized on demand, and it is kept for
/// the whole model, so each definition is finalized, and its errors are
/// reported, once.
pub struct FinalizeTypeDefs<'ast> {
    super_: TypeExpressionAnalyzer<'ast, Self>,
}

impl<'ast> Default for FinalizeTypeDefs<'ast> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> FinalizeTypeDefs<'ast> {
    pub fn new() -> FinalizeTypeDefs<'ast> {
        Self {
            super_: TypeExpressionAnalyzer::new(),
        }
    }
}

impl<'ast> Visitor<'ast> for FinalizeTypeDefs<'ast> {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    /// Expressions are not visited: a type name inside an expression (the
    /// operand of a `sizeof`) is not a use that this pass finalizes.
    fn visit_expr(&self, _: &mut Self::State, _: &'ast Expr) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }

    fn visit_type_name(&self, a: &mut Self::State, node: &'ast TypeName) -> ControlFlow<()> {
        self.type_name_node(a, node)?;
        ControlFlow::Continue(())
    }

    fn visit_def_alias_type(
        &self,
        a: &mut Self::State,
        node: &'ast DefAliasType,
    ) -> ControlFlow<Self::Break> {
        self.visit_if_needed(a, node, |a| {
            let def = a.interned_def(node);
            // Visit the referenced type
            let referenced_type = self.type_name_node(a, &node.type_name)?;
            // Update A in the type map
            a.assign_type(
                node.node_id,
                Arc::new(Type::Alias(AliasType {
                    node: def,
                    alias_type: referenced_type,
                })),
            );
            ControlFlow::Continue(())
        })
    }

    fn visit_def_array(
        &self,
        a: &mut Self::State,
        node: &'ast DefArray,
    ) -> ControlFlow<Self::Break> {
        self.visit_if_needed(a, node, |a| {
            let def = a.interned_def(node);
            // The type of this node before it is finalized
            let def_ty = a.type_map.get(&node.node_id).cloned();
            // Visit the element type of A, to update its members
            let elt_type = self.type_name_node(a, &node.elt_type)?;
            // Update the size and element type in A
            if a.get_big_int_value(node.size.node_id).is_none() {
                return ControlFlow::Continue(());
            }
            let size = match a.get_array_size(node.size.node_id, node.size.span()) {
                Ok(size) => size,
                Err(err) => {
                    err.emit();
                    return ControlFlow::Break(());
                }
            };
            let anon_array = AnonArrayType {
                size: Some(size as usize),
                elt_type: elt_type.clone(),
            };
            let mut array_type = ArrayType {
                node: def,
                anon_array,
                default: None,
                format: None,
            };
            // Compute the default value
            let array_type_arc = Arc::new(array_type.clone());
            let default = match &node.default {
                Some(default) => {
                    let ty = Arc::new(Type::Array(array_type_arc));
                    match Self::default_value(a, default, def_ty.as_ref(), &ty)? {
                        Some(Value::Array(v)) => Some(v),
                        _ => None,
                    }
                }
                None => {
                    match Arc::new(Type::AnonArray(array_type.anon_array.clone())).default_value() {
                        Some(Value::AnonArray(anon_array)) => Some(ArrayValue {
                            anon_array,
                            ty: array_type_arc,
                        }),
                        _ => None,
                    }
                }
            };
            // Compute the format
            let format = node
                .format
                .as_ref()
                .and_then(|format| Self::format(format, &elt_type));
            // Update the default value and format in A
            array_type.default = default;
            array_type.format = format;
            // Update A in the type map
            a.assign_type(node.node_id, Arc::new(Type::Array(Arc::new(array_type))));
            ControlFlow::Continue(())
        })
    }

    fn visit_def_enum(&self, a: &mut Self::State, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        self.visit_if_needed(a, node, |a| {
            // `CheckTypeUses` always enters an enum type for the definition node.
            let mut enum_type = match a.type_map.get(&node.node_id).unwrap().deref() {
                Type::Enum(ty) => ty.deref().clone(),
                _ => panic!("expected enum type"),
            };
            let default = match &node.default {
                Some(default) => a.value_map.get(&default.node_id).cloned(),
                None => match node.constants.first() {
                    None => None,
                    Some(first_constant) => a.value_map.get(&first_constant.node_id).cloned(),
                },
            };
            enum_type.default = match default {
                Some(Value::EnumConstant(v)) => Some(v),
                _ => None,
            };
            a.assign_type(node.node_id, Arc::new(Type::Enum(Arc::new(enum_type))));
            ControlFlow::Continue(())
        })
    }

    fn visit_def_struct(
        &self,
        a: &mut Self::State,
        node: &'ast DefStruct,
    ) -> ControlFlow<Self::Break> {
        self.visit_if_needed(a, node, |a| {
            let def = a.interned_def(node);
            // The type of this node before it is finalized
            let def_ty = a.type_map.get(&node.node_id).cloned();
            // Visit the anonymous struct type of S, to update its members
            let mut members = Vec::with_capacity(node.members.len());
            for member in &node.members {
                let member_type = self.type_name_node(a, &member.type_name)?;
                members.push((member.name.data.clone(), member_type));
            }
            // Update the anonymous struct type of S
            let mut struct_type = StructType {
                node: def,
                anon_struct: AnonStructType { members },
                default: None,
                sizes: Default::default(),
                formats: Default::default(),
            };
            // Compute the default value
            let struct_type_arc = Arc::new(struct_type.clone());
            let default = match &node.default {
                Some(default) => {
                    let ty = Arc::new(Type::Struct(struct_type_arc));
                    match Self::default_value(a, default, def_ty.as_ref(), &ty)? {
                        Some(Value::Struct(v)) => Some(v),
                        _ => None,
                    }
                }
                None => match Arc::new(Type::AnonStruct(struct_type.anon_struct.clone()))
                    .default_value()
                {
                    Some(Value::AnonStruct(anon_struct)) => Some(StructValue {
                        anon_struct,
                        ty: struct_type_arc,
                    }),
                    _ => None,
                },
            };
            // Compute the sizes
            for member in &node.members {
                let Some(size) = &member.size else { continue };
                if a.get_big_int_value(size.node_id).is_none() {
                    continue;
                }
                match a.get_array_size(size.node_id, size.span()) {
                    Ok(n) => {
                        struct_type.sizes.insert(member.name.data.clone(), n as u32);
                    }
                    Err(err) => {
                        err.emit();
                        return ControlFlow::Break(());
                    }
                }
            }
            // Compute the formats
            for (member, (_, member_type)) in node
                .members
                .iter()
                .zip(struct_type.anon_struct.members.iter())
            {
                if let Some(format) = member
                    .format
                    .as_ref()
                    .and_then(|format| Self::format(format, member_type))
                {
                    struct_type.formats.insert(member.name.data.clone(), format);
                }
            }
            // Update the default value, sizes, and formats in S
            struct_type.default = default;
            // Update S in the type map
            a.assign_type(node.node_id, Arc::new(Type::Struct(Arc::new(struct_type))));
            ControlFlow::Continue(())
        })
    }

    /// A module template is analyzed where it is expanded. At the definition,
    /// only the types of its constant parameters are visited.
    fn visit_def_module_template(
        &self,
        a: &mut Self::State,
        node: &'ast DefModuleTemplate,
    ) -> ControlFlow<Self::Break> {
        for param in &node.params {
            if let TemplateParam::Constant(param) = param {
                self.visit_type_name(a, &param.type_name)?;
            }
        }
        ControlFlow::Continue(())
    }
}

impl<'ast> FinalizeTypeDefs<'ast> {
    /// Runs `visitor` on a definition node unless the definition symbol is in
    /// the visited symbol set, and then adds the symbol to the set
    fn visit_if_needed<N: AstNode>(
        &self,
        a: &mut Analysis,
        node: &N,
        visitor: impl FnOnce(&mut Analysis) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        let Some(symbol) = a.get_symbol(node) else {
            return ControlFlow::Continue(());
        };
        if a.visited_symbol_set.contains(&symbol) {
            return ControlFlow::Continue(());
        }
        visitor(a)?;
        a.visited_symbol_set.insert(symbol);
        ControlFlow::Continue(())
    }

    /// Finalizes the type named by a type name, and records and returns it
    pub(crate) fn type_name_node(
        &self,
        a: &mut Analysis,
        node: &'ast TypeName,
    ) -> ControlFlow<(), Arc<Type>> {
        let ty = self.finalized_type(a, node)?;
        a.assign_type(node.node_id, ty.clone());
        ControlFlow::Continue(ty)
    }

    /// Computes the finalized type named by a type name, without recording it
    /// as the type of the type name: the definition the name refers to is
    /// finalized, and the size of a `string size n` type is evaluated and
    /// checked. This is Scala's TypeVisitor applied to the type of the type
    /// name; the type of a `string size n` type name does not hold the size
    /// expression, so the type is reached through the type name.
    pub(crate) fn finalized_type(
        &self,
        a: &mut Analysis,
        node: &'ast TypeName,
    ) -> ControlFlow<(), Arc<Type>> {
        let finalized = match &node.kind {
            TypeNameKind::QualIdent(q) => match a.use_def_map.get(&q.id()).cloned() {
                Some(Symbol::AliasType(def)) => {
                    self.visit_def_alias_type(a, def.deref())?;
                    a.type_map.get(&def.node_id).cloned()
                }
                Some(Symbol::ArrayType(def)) => {
                    self.visit_def_array(a, def.deref())?;
                    a.type_map.get(&def.node_id).cloned()
                }
                Some(Symbol::EnumType(def)) => {
                    self.visit_def_enum(a, def.deref())?;
                    a.type_map.get(&def.node_id).cloned()
                }
                Some(Symbol::StructType(def)) => {
                    self.visit_def_struct(a, def.deref())?;
                    a.type_map.get(&def.node_id).cloned()
                }
                // A type parameter names the type its argument names
                Some(Symbol::TemplateTypeArg { value, .. }) => {
                    Some(self.finalized_type(a, value.deref())?)
                }
                _ => None,
            },
            TypeNameKind::String(Some(size)) => match a.get_big_int_value(size.node_id) {
                Some(size_v) => {
                    if !Type::is_valid_string_size(size_v) {
                        SemanticError::InvalidStringSize {
                            loc: size.span(),
                            size: size_v,
                        }
                        .emit();
                        return ControlFlow::Break(());
                    }
                    Some(Arc::new(Type::String(Some(size_v))))
                }
                None => None,
            },
            _ => None,
        };

        // `CheckTypeUses` normally gives every type name it reaches an entry.
        // Fall back to the shared unknown type rather than assuming that: a
        // type name with no entry must degrade to an unknown type, not abort
        // the compiler (which would also take down `fpp_lsp_server`).
        let ty = match finalized.or_else(|| a.type_map.get(&node.node_id).cloned()) {
            Some(ty) => ty,
            None => a.unknown_type(node.span()),
        };
        ControlFlow::Continue(ty)
    }

    /// Computes a default value for a definition of type `ty`. The value of the
    /// default expression is converted to the type, which now has its array
    /// sizes; a default whose expression type does not convert to `def_ty`,
    /// the type of the definition before it was finalized, is reported by
    /// `CheckExprTypes` and not checked again here.
    fn default_value(
        a: &Analysis,
        default: &Expr,
        def_ty: Option<&Arc<Type>>,
        ty: &Arc<Type>,
    ) -> ControlFlow<(), Option<Value>> {
        let Some(v) = a.value_map.get(&default.node_id) else {
            return ControlFlow::Continue(None);
        };
        let checked = match (a.type_map.get(&default.node_id), def_ty) {
            (Some(expr_ty), Some(def_ty)) => Type::convert(expr_ty, def_ty).is_ok(),
            _ => true,
        };
        if checked && let Err(err) = Analysis::convert_types(default.span(), &v.get_type(), ty) {
            err.emit();
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(v.convert(ty))
    }

    /// Computes a format for the values of a type, reporting an invalid format
    /// string
    fn format(node: &LitString, ty: &Arc<Type>) -> Option<Format> {
        match Analysis::compute_format(node, std::slice::from_ref(ty)) {
            Ok(format) => Some(format),
            Err(err) => {
                err.emit();
                None
            }
        }
    }
}
