use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{UseAnalysisPass, visit_template_arg};
use crate::analyzers::use_analyzer::UseAnalyzer;
use crate::errors::{SemanticError, SemanticResult};
use crate::passes::FinalizeTypeDefs;
use crate::semantics::{
    AnonArrayValue, AnonStructValue, ArrayValue, BooleanValue, EnumConstantValue, FloatValue,
    IntegerValue, QualifiedName, SerializedSizeError, StringValue, StructValue, Symbol,
    SymbolInterface, Type, Value,
};
use fpp_ast::{
    Binop, DefConstant, DefEnum, DefEnumConstant, Expr, ExprKind, Ident, Node, StructExprMember,
    TransUnit, TypeName, Visitable, Visitor,
};
use fpp_core::{Span, Spanned};
use rustc_hash::FxHashMap as HashMap;
use std::ops::{ControlFlow, Deref};
use std::sync::Arc;

/// Compute the values of constants symbols and expressions
pub struct EvalConstantExprs<'ast> {
    super_: UseAnalyzer<'ast, Self>,
}

impl<'ast> Default for EvalConstantExprs<'ast> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> EvalConstantExprs<'ast> {
    pub fn new() -> EvalConstantExprs<'ast> {
        Self {
            super_: UseAnalyzer::new(),
        }
    }

    /// Evaluates the definition that a constant use resolves to, then copies its
    /// value to the use.
    fn eval_constant_use(&self, a: &mut Analysis, node: fpp_core::Node) -> ControlFlow<()> {
        let symbol = match a.use_def_map.get(&node) {
            Some(sym @ Symbol::Constant(def)) => {
                let sym = sym.clone();
                def.clone().visit(a, self)?;
                sym
            }
            Some(sym @ Symbol::EnumConstant(def)) => {
                let sym = sym.clone();
                def.clone().visit(a, self)?;
                sym
            }
            // Bound constant parameter: evaluate the bound argument to ensure
            // that it has a value
            Some(sym @ Symbol::TemplateConstantArg { value, .. }) => {
                let sym = sym.clone();
                if !a.value_map.contains_key(&value.node_id) {
                    self.template_constant_arg(a, &sym)?;
                }
                sym
            }
            _ => return ControlFlow::Continue(()),
        };

        if let Some(value) = a.value_map.get(&symbol.node()).cloned() {
            a.assign_value(node, value);
        }

        ControlFlow::Continue(())
    }

    /// Whether `ty` is finalized: either it has a definition symbol that has
    /// been visited, or it has no definition symbol at all.
    fn type_is_finalized(a: &Analysis, ty: &Arc<Type>) -> bool {
        match ty.def_symbol() {
            Some(symbol) => a.visited_symbol_set.contains(&symbol),
            None => true,
        }
    }

    /// Finalizes `ty` unless it is already finalized.
    fn finalize_if_needed(&self, a: &mut Analysis, ty: &Arc<Type>) -> ControlFlow<()> {
        if Self::type_is_finalized(a, ty) {
            return ControlFlow::Continue(());
        }
        self.finalize_type(a, ty)
    }

    /// Evaluates a type definition with this pass and then hands it to
    /// `FinalizeTypeDefs`, recursing into element and member types
    fn finalize_type(&self, a: &mut Analysis, ty: &Arc<Type>) -> ControlFlow<()> {
        let finalize_defs = FinalizeTypeDefs::new();
        match ty.deref() {
            Type::Alias(alias) => {
                let def = alias.node.clone();
                self.visit_def_alias_type(a, &def)?;
                self.finalize_if_needed(a, &alias.alias_type)?;
                finalize_defs.visit_def_alias_type(a, &def)?;
            }
            Type::Array(array) => {
                let def = array.node.clone();
                self.visit_def_array(a, &def)?;
                self.finalize_if_needed(a, &array.anon_array.elt_type)?;
                finalize_defs.visit_def_array(a, &def)?;
            }
            Type::Enum(enum_ty) => {
                let def = enum_ty.node.clone();
                self.visit_def_enum(a, &def)?;
                finalize_defs.visit_def_enum(a, &def)?;
            }
            Type::Struct(struct_ty) => {
                let def = struct_ty.node.clone();
                self.visit_def_struct(a, &def)?;
                for (_, member_type) in &struct_ty.anon_struct.members {
                    self.finalize_if_needed(a, member_type)?;
                }
                finalize_defs.visit_def_struct(a, &def)?;
            }
            _ => {}
        }

        ControlFlow::Continue(())
    }

    fn expr_array_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        elts: &'ast [Expr],
    ) -> ControlFlow<()> {
        self.super_visit(a, Node::Expr(node))?;
        let elt_type = match a.type_map[&node.node_id].deref() {
            Type::AnonArray(anon_array) => anon_array.elt_type.clone(),
            // A named array type here means the array expression is
            // the argument bound to a constant template parameter,
            // whose type is the one declared in the parameter
            Type::Array(array) => array.anon_array.elt_type.clone(),
            _ => panic!("element type of array expression should be AnonArray or Array"),
        };
        let mut values = Vec::with_capacity(elts.len());
        for elt in elts {
            // An element with no value failed to evaluate
            match a
                .value_map
                .get(&elt.node_id)
                .and_then(|v| v.convert(&elt_type))
            {
                Some(v) => values.push(v),
                None => return ControlFlow::Continue(()),
            }
        }
        a.assign_value(node.node_id, Value::AnonArray(AnonArrayValue::new(values)));
        ControlFlow::Continue(())
    }

    fn expr_array_subscript_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        e1: &'ast Expr,
        e2: &'ast Expr,
    ) -> ControlFlow<()> {
        self.super_visit(a, Node::Expr(node))?;

        let anon_array = match a.value_map.get(&e1.node_id) {
            Some(Value::AnonArray(anon_array))
            | Some(Value::Array(ArrayValue { anon_array, .. })) => anon_array,
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
        // If the size of the array is not yet known, then we can check only the
        // lower bound here
        if index < 0 {
            SemanticError::InvalidIntValue {
                loc: e2.span(),
                v: Some(index),
                msg: "value may not be negative".to_string(),
            }
            .emit();
            return ControlFlow::Break(());
        } else if anon_array.scalar.is_none() && index >= anon_array.elements.len() as i128 {
            // Compare in i128: narrowing the index to usize first would
            // accept any index congruent to an in-bounds one modulo the
            // pointer width.
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

        let v = match &anon_array.scalar {
            Some(scalar) => scalar.deref().clone(),
            None => anon_array.get(index as usize).expect("in bounds").clone(),
        };
        a.assign_value(node.node_id, v);
        ControlFlow::Continue(())
    }

    fn expr_binop_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        e1: &'ast Expr,
        op: &Binop,
        e2: &'ast Expr,
    ) -> ControlFlow<()> {
        self.super_visit(a, Node::Expr(node))?;
        // An operand with no value failed to evaluate
        if !a.value_map.contains_key(&e1.node_id) || !a.value_map.contains_key(&e2.node_id) {
            return ControlFlow::Continue(());
        }
        let (id1, id2, loc) = (e1.node_id, e2.node_id, node.span());
        let v = match op {
            Binop::Add => a.add(id1, id2, loc),
            Binop::Div => a.div(id1, id2, loc),
            Binop::Mul => a.mul(id1, id2, loc),
            Binop::Sub => a.sub(id1, id2, loc),
            Binop::LShift => a.lshift(id1, id2, loc),
            Binop::RShift => a.rshift(id1, id2, loc),
        };
        Self::assign_value_result(a, node, v)
    }

    fn expr_literal_bool_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        b: bool,
    ) -> ControlFlow<()> {
        let v = Value::Boolean(BooleanValue(b));
        a.assign_value(node.node_id, v);
        ControlFlow::Continue(())
    }

    fn expr_literal_float_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        value: &str,
    ) -> ControlFlow<()> {
        let value: f64 = match value.parse() {
            Ok(value) => value,
            Err(err) => {
                SemanticError::InvalidIntValue {
                    loc: node.span(),
                    v: None,
                    msg: format!("failed to parse floating value: {}", err),
                }
                .emit();
                return ControlFlow::Break(());
            }
        };
        let v = Value::Float(FloatValue {
            value,
            kind: fpp_ast::FloatKind::F64,
        });
        a.assign_value(node.node_id, v);
        ControlFlow::Continue(())
    }

    /// Integer values are `i128`, so a literal that does not fit is an error
    fn expr_literal_int_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        value: &str,
    ) -> ControlFlow<()> {
        let int_value = if value.starts_with("0x") || value.starts_with("0X") {
            match i128::from_str_radix(&value[2..], 16) {
                Ok(v) => v,
                Err(err) => {
                    SemanticError::InvalidIntValue {
                        loc: node.span(),
                        v: None,
                        msg: format!("failed to parse hexadecimal integral value: {}", err),
                    }
                    .emit();
                    return ControlFlow::Break(());
                }
            }
        } else {
            match value.parse() {
                Ok(v) => v,
                Err(err) => {
                    SemanticError::InvalidIntValue {
                        loc: node.span(),
                        v: None,
                        msg: format!("failed to parse integral value: {}", err),
                    }
                    .emit();
                    return ControlFlow::Break(());
                }
            }
        };
        a.assign_value(node.node_id, Value::Integer(IntegerValue(int_value)));
        ControlFlow::Continue(())
    }

    fn expr_literal_string_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        value: &str,
    ) -> ControlFlow<()> {
        let v = Value::String(StringValue(value.to_string()));
        a.assign_value(node.node_id, v);
        ControlFlow::Continue(())
    }

    fn expr_paren_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        e: &'ast Expr,
    ) -> ControlFlow<()> {
        self.super_visit(a, Node::Expr(node))?;
        if let Some(v) = a.value_map.get(&e.node_id).cloned() {
            a.assign_value(node.node_id, v);
        }
        ControlFlow::Continue(())
    }

    fn expr_size_of_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        type_name: &'ast TypeName,
    ) -> ControlFlow<()> {
        self.super_visit(a, Node::Expr(node))?;
        // Finalize the type so we can compute the size
        if let Some(ty) = a.type_map.get(&type_name.node_id).cloned() {
            self.finalize_if_needed(a, &ty)?;
        }
        // A `string size N` type name has no definition to evaluate,
        // but its size still has to be resolved into the type.
        FinalizeTypeDefs::new().ty(a, type_name)?;
        if let Some(ty) = a.type_map.get(&type_name.node_id).cloned() {
            // Get the finalized type. For a type with a definition,
            // the finalized type is mapped to the definition in the
            // type map; otherwise the type is already the finalized
            // type.
            let finalized = match ty.def_node_id() {
                Some(def_node) => a.type_map.get(&def_node).cloned().unwrap_or(ty),
                None => ty,
            };
            // Use the finalized type to compute the size
            match finalized.serialized_size(a) {
                Ok(size) => {
                    a.assign_value(node.node_id, Value::Integer(IntegerValue(size)));
                }
                // The size is unknown here; a type whose size is never
                // knowable is reported by `CheckExprTypes`
                Err(SerializedSizeError::Unavailable) => {}
                Err(SerializedSizeError::TooLarge) => {
                    SemanticError::InvalidIntValue {
                        loc: node.span(),
                        v: None,
                        msg: "serialized size is too large to represent".to_string(),
                    }
                    .emit();
                    return ControlFlow::Break(());
                }
            }
        }
        ControlFlow::Continue(())
    }

    fn expr_struct_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        members: &'ast [StructExprMember],
    ) -> ControlFlow<()> {
        self.super_visit(a, Node::Expr(node))?;
        let mut member_values = HashMap::default();
        for member in members {
            match a.value_map.get(&member.value.node_id) {
                Some(v) => {
                    member_values.insert(member.name.data.clone(), v.clone());
                }
                // A member with no value failed to evaluate
                None => return ControlFlow::Continue(()),
            }
        }
        let v = Value::AnonStruct(AnonStructValue {
            members: member_values,
        });
        a.assign_value(node.node_id, v);
        ControlFlow::Continue(())
    }

    fn expr_unop_node(&self, a: &mut Analysis, node: &'ast Expr, e: &'ast Expr) -> ControlFlow<()> {
        self.super_visit(a, Node::Expr(node))?;
        if !a.value_map.contains_key(&e.node_id) {
            return ControlFlow::Continue(());
        }
        let v = a.neg(e.node_id, node.span());
        Self::assign_value_result(a, node, v)
    }

    fn expr_dot_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        e: &'ast Expr,
        id: &'ast Ident,
    ) -> ControlFlow<()> {
        // Ensure that the parent selector has a type
        self.super_visit(a, Node::Expr(node))?;

        // Get the value of the selected member
        if a.value_map.contains_key(&node.node_id) {
            // If the entire dot expression was already resolved by
            // a constant_use, the value will already be in this map
            return ControlFlow::Continue(());
        }
        // The value is not already resolved, this is some sort of
        // member selection.
        let member_value = match a.value_map.get(&e.node_id) {
            None => return ControlFlow::Continue(()),
            Some(Value::AnonStruct(anon_struct))
            | Some(Value::Struct(StructValue { anon_struct, .. })) => {
                anon_struct.members.get(&id.data).cloned()
            }
            Some(v) => {
                SemanticError::InvalidTypeForMemberSelection {
                    member_name: id.data.clone(),
                    loc: id.span(),
                    type_name: v.get_type().to_string(),
                }
                .emit();
                return ControlFlow::Break(());
            }
        };
        if let Some(v) = member_value {
            a.assign_value(node.node_id, v);
        }
        ControlFlow::Continue(())
    }

    /// Record the value of an expression, or report the error that computing
    /// it raised
    fn assign_value_result(
        a: &mut Analysis,
        node: &'ast Expr,
        v: SemanticResult<Value>,
    ) -> ControlFlow<()> {
        match v {
            Ok(v) => {
                a.assign_value(node.node_id, v);
                ControlFlow::Continue(())
            }
            Err(err) => {
                err.emit();
                ControlFlow::Break(())
            }
        }
    }
}

impl<'ast> Visitor<'ast> for EvalConstantExprs<'ast> {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    fn visit_trans_units(
        &self,
        a: &mut Self::State,
        i: impl Iterator<Item = &'ast TransUnit>,
    ) -> ControlFlow<Self::Break> {
        // The visited symbol set records the type definitions that this pass
        // finalizes on demand. CheckUseDefCycles leaves the symbols it visits
        // in the set, so the pass starts with an empty set.
        a.visited_symbol_set.clear();
        for tu in i {
            self.visit_trans_unit(a, tu)?;
        }
        ControlFlow::Continue(())
    }

    fn visit_def_constant(
        &self,
        a: &mut Self::State,
        node: &'ast DefConstant,
    ) -> ControlFlow<Self::Break> {
        if a.value_map.contains_key(&node.node_id) {
            return ControlFlow::Continue(());
        }

        self.super_visit(a, Node::DefConstant(node))?;
        if let Some(v) = a.value_map.get(&node.value.node_id).cloned() {
            a.assign_value(node.node_id, v);
        }

        ControlFlow::Continue(())
    }

    fn visit_def_enum(&self, a: &mut Self::State, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        fn check_for_duplicate_value(
            a: &Analysis,
            constants: &[DefEnumConstant],
        ) -> SemanticResult {
            let mut values: HashMap<i128, Span> = HashMap::default();
            for constant in constants {
                // A constant with no value failed to evaluate
                if let Some(Value::EnumConstant(EnumConstantValue { value, .. })) =
                    a.value_map.get(&constant.node_id)
                    && let Some(prev_loc) = values.insert(value.1, constant.span())
                {
                    return Err(SemanticError::DuplicateEnumValue {
                        value: value.1.to_string(),
                        loc: constant.span(),
                        prev_loc,
                    });
                }
            }
            Ok(())
        }

        self.super_visit(a, Node::DefEnum(node))?;
        match check_for_duplicate_value(a, &node.constants) {
            Ok(()) => ControlFlow::Continue(()),
            Err(err) => {
                err.emit();
                ControlFlow::Break(())
            }
        }
    }

    fn visit_def_enum_constant(
        &self,
        a: &mut Self::State,
        node: &'ast DefEnumConstant,
    ) -> ControlFlow<Self::Break> {
        if a.value_map.contains_key(&node.node_id) {
            return ControlFlow::Continue(());
        }
        // An implied enum constant with no value belongs to an enum whose
        // constants are invalid, and that error is already reported
        let Some(e) = &node.value else {
            return ControlFlow::Continue(());
        };

        self.super_visit(a, Node::DefEnumConstant(node))?;
        // A value that does not convert to Integer does not have a numeric
        // type, and that error is already reported
        let int_value = match a
            .value_map
            .get(&e.node_id)
            .and_then(|v| v.convert(&Arc::new(Type::Integer)))
        {
            Some(Value::Integer(IntegerValue(int_value))) => int_value,
            _ => return ControlFlow::Continue(()),
        };
        let enum_type = match a.type_map.get(&node.node_id).map(|t| t.deref()) {
            Some(Type::Enum(enum_type)) => enum_type.clone(),
            _ => return ControlFlow::Continue(()),
        };
        let enum_value = EnumConstantValue::new(node.name.data.clone(), int_value, enum_type);
        a.assign_value(node.node_id, Value::EnumConstant(enum_value));
        ControlFlow::Continue(())
    }

    fn visit_expr(&self, a: &mut Self::State, node: &'ast Expr) -> ControlFlow<Self::Break> {
        // If we've already visited the expression, don't analyze it again
        if a.value_map.contains_key(&node.node_id) {
            return ControlFlow::Continue(());
        }
        // An expression with no type failed to type check, and that error is
        // already reported, so it has no value
        if !a.type_map.contains_key(&node.node_id) {
            return ControlFlow::Continue(());
        }

        match &node.kind {
            ExprKind::Array(elts) => self.expr_array_node(a, node, elts),
            ExprKind::ArraySubscript { e1, e2 } => self.expr_array_subscript_node(a, node, e1, e2),
            ExprKind::Binop { left, op, right } => self.expr_binop_node(a, node, left, op, right),
            ExprKind::Dot { e, id } => self.expr_dot_node(a, node, e, id),
            // An identifier is a constant use, evaluated by constant_use
            ExprKind::Ident { .. } => self.super_visit(a, Node::Expr(node)),
            ExprKind::LiteralBool(b) => self.expr_literal_bool_node(a, node, *b),
            ExprKind::LiteralFloat(value) => self.expr_literal_float_node(a, node, value),
            ExprKind::LiteralInt(value) => self.expr_literal_int_node(a, node, value),
            ExprKind::LiteralString(value) => self.expr_literal_string_node(a, node, value),
            ExprKind::Paren(e) => self.expr_paren_node(a, node, e),
            ExprKind::SizeOf(type_name) => self.expr_size_of_node(a, node, type_name),
            ExprKind::Struct(members) => self.expr_struct_node(a, node, members),
            ExprKind::Unop { e, .. } => self.expr_unop_node(a, node, e),
        }
    }
}

impl<'ast> UseAnalysisPass<'ast, Analysis> for EvalConstantExprs<'ast> {
    fn constant_use(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.eval_constant_use(a, node.node_id)
    }

    fn implied_constant_use(
        &self,
        a: &mut Analysis,
        node: &Expr,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.eval_constant_use(a, node.node_id)
    }

    /// The value of a bound constant parameter is the value of the argument
    /// bound to it, converted to the type declared in the parameter.
    fn template_constant_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        let Symbol::TemplateConstantArg { param_def, value } = arg else {
            return ControlFlow::Continue(());
        };
        let param_def = param_def.clone();
        let value = value.clone();
        let loc = value.span();

        // Compute the value of the argument
        visit_template_arg(self, a, arg)?;

        // The type of the parameter may name an array or struct whose size or
        // default value is not known until it is finalized
        if let Some(ty) = a.type_map.get(&param_def.type_name.node_id).cloned() {
            self.finalize_if_needed(a, &ty)?;
        }
        let ty = FinalizeTypeDefs::new().ty(a, &param_def.type_name)?;

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
