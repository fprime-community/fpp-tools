use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{UseAnalysisPass, visit_template_arg};
use crate::analyzers::use_analyzer::UseAnalyzer;
use crate::errors::SemanticError;
use crate::semantics::{
    AnonArrayType, AnonArrayValue, AnonStructType, AnonStructValue, ArrayValue, IntegerValue,
    QualifiedName, StructValue, Symbol, SymbolInterface, Type, Value,
};
use fpp_ast::{DefConstant, Expr, ExprKind, Node, Visitable, Visitor};
use fpp_core::Spanned;
use std::ops::{ControlFlow, Deref};
use std::sync::Arc;

/// Finalize constant expressions.
///
/// Recompute the types and values of constant expressions, using the finalized
/// type definitions. Doing this fills in any missing anonymous array sizes and
/// updates any type or value that depends on a finalized type definition.
pub struct FinalizeConstantExprs<'ast> {
    super_: UseAnalyzer<'ast, Self>,
}

impl<'ast> Default for FinalizeConstantExprs<'ast> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> FinalizeConstantExprs<'ast> {
    pub fn new() -> FinalizeConstantExprs<'ast> {
        Self {
            super_: UseAnalyzer::new(),
        }
    }

    /// Recompute the type and value of an array expression
    fn expr_array_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        elts: &'ast [Expr],
    ) -> ControlFlow<()> {
        let empty_list_error = SemanticError::EmptyArray { loc: node.span() };
        let ids: Vec<fpp_core::Node> = elts.iter().map(|e| e.node_id).collect();
        let elt_type = match a.common_type(&ids, empty_list_error) {
            Ok(ty) => ty,
            Err(err) => {
                err.emit();
                return ControlFlow::Break(());
            }
        };

        // An array element type whose size is unknown means the elements are
        // arrays of sizes that do not agree
        if let Type::AnonArray(AnonArrayType { size: None, .. }) = elt_type.deref() {
            let type_names: Vec<String> = elts
                .iter()
                .filter_map(|e| a.type_map.get(&e.node_id))
                .map(|ty| ty.to_string())
                .collect();
            SemanticError::TypeMismatch {
                loc: node.span(),
                msg: format!(
                    "cannot compute common type of array value with elements: {}",
                    type_names.join(", ")
                ),
                reason: None,
            }
            .emit();
            return ControlFlow::Break(());
        }

        let mut values = Vec::with_capacity(elts.len());
        for elt in elts {
            match a
                .value_map
                .get(&elt.node_id)
                .and_then(|v| v.convert(&elt_type))
            {
                Some(v) => values.push(v),
                None => return ControlFlow::Continue(()),
            }
        }

        a.assign_type(
            node.node_id,
            Arc::new(Type::AnonArray(AnonArrayType {
                size: Some(elts.len()),
                elt_type,
            })),
        );
        a.assign_value(node.node_id, Value::AnonArray(AnonArrayValue::new(values)));
        ControlFlow::Continue(())
    }

    /// Recompute the type and value of an array subscript expression
    fn expr_array_subscript_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        e1: &'ast Expr,
        e2: &'ast Expr,
    ) -> ControlFlow<()> {
        let anon_array = match a.value_map.get(&e1.node_id) {
            Some(Value::AnonArray(anon_array))
            | Some(Value::Array(ArrayValue { anon_array, .. })) => anon_array.clone(),
            _ => return ControlFlow::Continue(()),
        };

        // The index is converted to Integer (spec, Array Subscript Expressions)
        let index = match a
            .value_map
            .get(&e2.node_id)
            .and_then(|v| v.convert(&Arc::new(Type::Integer)))
        {
            Some(Value::Integer(IntegerValue(index))) => index,
            _ => return ControlFlow::Continue(()),
        };

        // Check if the index is in bounds
        // For a scalar value promoted to an array, the size is not known until
        // the array type is finalized, so this is the first place where the upper
        // bound can be checked
        if index < 0 {
            SemanticError::InvalidIntValue {
                loc: e2.span(),
                v: Some(index),
                msg: "value may not be negative".to_string(),
            }
            .emit();
            return ControlFlow::Break(());
        }
        if index >= anon_array.elements.len() as i128 {
            SemanticError::InvalidIntValue {
                loc: e2.span(),
                v: Some(index),
                msg: format!(
                    "index value is not in the range [0, {}]",
                    anon_array.elements.len().saturating_sub(1)
                ),
            }
            .emit();
            return ControlFlow::Break(());
        }

        // Update the element type, in case the type of the array was finalized
        let elt_type = match a.type_map.get(&e1.node_id).map(Type::underlying_type) {
            Some(ty) => match ty.deref() {
                Type::AnonArray(anon) => Some(anon.elt_type.clone()),
                Type::Array(arr) => Some(arr.anon_array.elt_type.clone()),
                _ => None,
            },
            None => None,
        };
        if let Some(elt_type) = elt_type {
            a.assign_type(node.node_id, elt_type);
        }
        let element = anon_array.get(index as usize).expect("in bounds").clone();
        a.assign_value(node.node_id, element);
        ControlFlow::Continue(())
    }

    /// Recompute the type and value of a struct member selection
    fn expr_dot_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        e: &'ast Expr,
        id: &str,
    ) -> ControlFlow<()> {
        // If this node is a use of a symbol, then visiting it has already updated
        // its type and value. Otherwise it selects a member of a struct value, so
        // update the type and value of the selection.
        if a.use_def_map.contains_key(&node.node_id) {
            return ControlFlow::Continue(());
        }
        let member_ty = a
            .type_map
            .get(&e.node_id)
            .map(Type::underlying_type)
            .and_then(|ty| ty.as_anon_struct().and_then(|s| s.get_member(id)).cloned());
        let member_value = match a.value_map.get(&e.node_id) {
            Some(Value::Struct(StructValue { anon_struct, .. }))
            | Some(Value::AnonStruct(anon_struct)) => anon_struct.members.get(id).cloned(),
            _ => None,
        };
        if let Some(ty) = member_ty {
            a.assign_type(node.node_id, ty);
        }
        if let Some(v) = member_value {
            a.assign_value(node.node_id, v);
        }
        ControlFlow::Continue(())
    }

    /// Recompute the type and value of a struct expression
    fn expr_struct_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        members: &'ast [fpp_ast::StructExprMember],
    ) -> ControlFlow<()> {
        let mut type_members = Vec::with_capacity(members.len());
        let mut value_members = indexmap::IndexMap::new();
        for member in members {
            if let Some(ty) = a.type_map.get(&member.value.node_id) {
                type_members.push((member.name.data.clone(), ty.clone()));
            }
            if let Some(v) = a.value_map.get(&member.value.node_id).cloned() {
                value_members.insert(member.name.data.clone(), v);
            }
        }
        a.assign_type(
            node.node_id,
            Arc::new(Type::AnonStruct(AnonStructType {
                members: type_members,
            })),
        );
        a.assign_value(
            node.node_id,
            Value::AnonStruct(AnonStructValue {
                members: value_members,
            }),
        );
        ControlFlow::Continue(())
    }

    /// Recompute the type and value of a use of a constant symbol
    fn constant_use_node(&self, a: &mut Analysis, node: fpp_core::Node) -> ControlFlow<()> {
        let Some(symbol) = a.use_def_map.get(&node).cloned() else {
            return ControlFlow::Continue(());
        };
        match &symbol {
            // Constant symbol: visit the constant definition to recompute its
            // type and value
            Symbol::Constant(def) => {
                def.visit(a, self)?;
            }
            // Enum symbol: if this is in scope, then we are in the enum
            // definition, so it already has a type and value
            Symbol::EnumConstant(_) => {}
            Symbol::TemplateConstantArg { .. } => {
                self.template_constant_arg(a, &symbol)?;
            }
            // Invalid use of a symbol in an expression
            _ => {
                Analysis::invalid_symbol(&symbol, node, "not a constant symbol").emit();
                return ControlFlow::Break(());
            }
        }

        let id = symbol.node();
        if let Some(ty) = a.type_map.get(&id).cloned() {
            a.assign_type(node, ty);
        }
        if let Some(v) = a.value_map.get(&id).cloned() {
            a.assign_value(node, v);
        }
        ControlFlow::Continue(())
    }
}

impl<'ast> Visitor<'ast> for FinalizeConstantExprs<'ast> {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    fn visit_expr(&self, a: &mut Analysis, node: &'ast Expr) -> ControlFlow<Self::Break> {
        // An expression with no value failed to type check or to evaluate, and
        // that error is already reported
        if !a.value_map.contains_key(&node.node_id) {
            return ControlFlow::Continue(());
        }
        self.super_visit(a, Node::Expr(node))?;
        match &node.kind {
            ExprKind::Array(elts) => self.expr_array_node(a, node, elts),
            ExprKind::ArraySubscript { e1, e2 } => self.expr_array_subscript_node(a, node, e1, e2),
            ExprKind::Dot { e, id } => self.expr_dot_node(a, node, e, &id.data),
            ExprKind::Struct(members) => self.expr_struct_node(a, node, members),
            // An identifier use was updated by constant_use during super_visit,
            // and every other kind of expression keeps the type and value it had
            // before the type definitions were finalized
            _ => ControlFlow::Continue(()),
        }
    }

    fn visit_def_constant(
        &self,
        a: &mut Analysis,
        node: &'ast DefConstant,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::DefConstant(node))?;
        if let Some(ty) = a.type_map.get(&node.value.node_id).cloned() {
            a.assign_type(node.node_id, ty);
        }
        if let Some(v) = a.value_map.get(&node.value.node_id).cloned() {
            a.assign_value(node.node_id, v);
        }
        ControlFlow::Continue(())
    }
}

impl<'ast> UseAnalysisPass<'ast> for FinalizeConstantExprs<'ast> {
    fn constant_use(
        &self,
        a: &mut Analysis,
        node: &Expr,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.constant_use_node(a, node.node_id)
    }

    fn template_constant_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        let Symbol::TemplateConstantArg { param_def, value } = arg else {
            return ControlFlow::Continue(());
        };
        let param_def = param_def.clone();
        let value = value.clone();
        let loc = value.span();

        visit_template_arg(self, a, arg)?;

        let Some(ty) = a.type_map.get(&param_def.type_name.node_id).cloned() else {
            return ControlFlow::Continue(());
        };
        let Some(v) = a.value_map.get(&value.node_id).cloned() else {
            return ControlFlow::Continue(());
        };
        match v.convert(&ty) {
            Some(new_value) => {
                a.assign_type(value.node_id, ty);
                a.assign_value(value.node_id, new_value);
                ControlFlow::Continue(())
            }
            None => {
                SemanticError::TypeMismatch {
                    loc,
                    msg: format!("cannot convert value {} to type {}", v, ty),
                    reason: None,
                }
                .emit();
                // The bound parameter has no usable value, so every use of it
                // would report a further error
                ControlFlow::Break(())
            }
        }
    }

    fn template_type_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        visit_template_arg(self, a, arg)
    }
}
