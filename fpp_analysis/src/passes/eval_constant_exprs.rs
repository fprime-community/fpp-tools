use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::UseAnalysisPass;
use crate::analyzers::use_analyzer::UseAnalyzer;
use crate::errors::SemanticError;
use crate::passes::FinalizeTypeDefs;
use crate::semantics::{
    AnonArrayValue, AnonStructValue, ArrayValue, BooleanValue, EnumConstantValue, FloatValue,
    IntegerValue, MathError, PrimitiveIntegerValue, QualifiedName, SerializedSizeError,
    StringValue, StructValue, Symbol, SymbolInterface, Type, Value,
};
use fpp_ast::{
    Binop, DefConstant, DefEnum, DefEnumConstant, Expr, ExprKind, Node, TransUnit, Unop, Visitable,
    Visitor, Walkable,
};
use fpp_core::Spanned;
use rustc_hash::FxHashMap as HashMap;
use std::ops::{ControlFlow, Deref};
use std::sync::Arc;

/// Compute the values of constant symbols and expressions
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
}

impl<'ast> Visitor<'ast> for EvalConstantExprs<'ast> {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    fn visit_trans_unit(
        &self,
        a: &mut Self::State,
        node: &'ast TransUnit,
    ) -> ControlFlow<Self::Break> {
        // Reset the visited-symbol set so on-demand type finalization (for
        // `sizeof`) can run; it is repopulated by `FinalizeTypeDefs` afterward.
        a.visited_symbol_set.clear();
        node.walk(a, self)
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
        match a.value_map.get(&node.value.node_id) {
            None => {}
            Some(value) => {
                a.value_map.insert(node.node_id, value.clone());
            }
        }

        ControlFlow::Continue(())
    }

    fn visit_def_enum(&self, a: &mut Self::State, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::DefEnum(node))?;

        // Check for duplicate values
        let mut values: HashMap<i128, fpp_core::Span> = HashMap::default();
        for constant in &node.constants {
            if let Some(Value::EnumConstant(EnumConstantValue { value, .. })) =
                a.value_map.get(&constant.node_id)
                && let Some(old) = values.insert(value.1, constant.span())
            {
                SemanticError::DuplicateEnumConstant {
                    value: value.1,
                    loc: constant.span(),
                    prev_loc: old,
                }
                .emit()
            }
        }

        ControlFlow::Continue(())
    }

    fn visit_def_enum_constant(
        &self,
        a: &mut Self::State,
        node: &'ast DefEnumConstant,
    ) -> ControlFlow<Self::Break> {
        if a.value_map.contains_key(&node.node_id) {
            return ControlFlow::Continue(());
        }

        self.super_visit(a, Node::DefEnumConstant(node))?;

        fn apply_value(a: &mut Analysis, node: &DefEnumConstant) -> Option<()> {
            let value_expr = node.value.as_ref()?;

            let value = match a
                .value_map
                .get(&value_expr.node_id)?
                .convert(&Arc::new(Type::Integer))?
            {
                Value::Integer(IntegerValue(value)) => value,
                _ => panic!("expected integer value"),
            };

            let ty = a.type_map.get(&node.node_id)?;
            a.value_map.insert(
                node.node_id,
                Value::EnumConstant(EnumConstantValue::new(
                    node.name.data.clone(),
                    value,
                    ty.clone(),
                )),
            );

            Some(())
        }

        let _ = apply_value(a, node);
        ControlFlow::Continue(())
    }

    fn visit_expr(&self, a: &mut Self::State, node: &'ast Expr) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::Expr(node))?;

        match &node.kind {
            ExprKind::Array(e) => {
                let elt_type = match a.type_map.get(&node.node_id) {
                    None => return ControlFlow::Continue(()),
                    Some(ty) => match ty.deref() {
                        Type::AnonArray(arr) => arr.elt_type.clone(),
                        _ => panic!("type of array expression should be AnonArray"),
                    },
                };

                let mut out = vec![];
                for element in e {
                    // Look up the value and convert it to the element type
                    let val = match a.value_map.get(&element.node_id) {
                        None => return ControlFlow::Continue(()),
                        Some(val) => match val.convert(&elt_type) {
                            None => return ControlFlow::Continue(()),
                            Some(val) => val,
                        },
                    };

                    out.push(val.clone())
                }

                a.value_map
                    .insert(node.node_id, Value::AnonArray(AnonArrayValue::new(out)));
            }
            ExprKind::ArraySubscript { e1, e2 } => {
                let elements = match a.value_map.get(&e1.node_id) {
                    Some(Value::AnonArray(anon_array))
                    | Some(Value::Array(ArrayValue { anon_array, .. })) => anon_array,
                    _ => return ControlFlow::Continue(()),
                };

                let index = match a.value_map.get(&e2.node_id) {
                    None => return ControlFlow::Continue(()),
                    Some(Value::PrimitiveInteger(PrimitiveIntegerValue { value, .. })) => *value,
                    Some(Value::Integer(IntegerValue(value))) => *value,
                    _ => return ControlFlow::Continue(()),
                };

                // Check if the index is in bounds
                if index < 0 {
                    SemanticError::InvalidIntValue {
                        loc: e2.span(),
                        v: Some(index),
                        msg: "index value may not be negative".to_string(),
                    }
                    .emit();
                } else if index >= elements.elements.len() as i128 {
                    // Compare in i128, as Scala compares in BigInt: narrowing the
                    // index to usize first would accept any index congruent to an
                    // in-bounds one modulo the pointer width.
                    SemanticError::InvalidIntValue {
                        loc: e2.span(),
                        v: Some(index),
                        msg: format!(
                            "index value is not in the range [0, {}]",
                            elements.elements.len().saturating_sub(1)
                        ),
                    }
                    .emit();
                } else {
                    let element = elements.get(index as usize).expect("in bounds").clone();
                    a.value_map.insert(node.node_id, element);
                }
            }
            ExprKind::Binop { left, right, op } => {
                let left_val = match a.value_map.get(&left.node_id) {
                    None => return ControlFlow::Continue(()),
                    Some(v) => v,
                };

                let right_val = match a.value_map.get(&right.node_id) {
                    None => return ControlFlow::Continue(()),
                    Some(v) => v,
                };

                let val = match op {
                    Binop::LShift | Binop::RShift => {
                        // A shift checks its amount before applying the operator.
                        // Both operands must be integers; a non-integer operand has
                        // already been reported by the type checker.
                        let (Some(_), Some(shift)) =
                            (left_val.as_shift_int(), right_val.as_shift_int())
                        else {
                            return ControlFlow::Continue(());
                        };

                        if !(0..=255).contains(&shift) {
                            SemanticError::InvalidShiftAmount { loc: right.span() }.emit();
                            return ControlFlow::Continue(());
                        }

                        match op {
                            Binop::LShift => left_val.shl(right_val),
                            _ => left_val.shr(right_val),
                        }
                    }
                    Binop::Add => left_val.add(right_val),
                    Binop::Div => left_val.div(right_val),
                    Binop::Mul => left_val.mul(right_val),
                    Binop::Sub => left_val.sub(right_val),
                };

                match val {
                    Ok(val) => {
                        a.value_map.insert(node.node_id, val);
                    }
                    Err(MathError::DivByZero) => {
                        SemanticError::DivisionByZero { loc: right.span() }.emit();
                    }
                    Err(MathError::ShiftOverflow) => {
                        SemanticError::InvalidIntValue {
                            loc: node.span(),
                            v: None,
                            msg: "shift result is too large to represent".to_string(),
                        }
                        .emit();
                    }
                    Err(MathError::Overflow) => {
                        SemanticError::InvalidIntValue {
                            loc: node.span(),
                            v: None,
                            msg: "arithmetic result is too large to represent".to_string(),
                        }
                        .emit();
                    }
                    Err(MathError::InvalidInputs) => {}
                }
            }
            ExprKind::Dot { e, id } => {
                match a.value_map.get(&node.node_id) {
                    None => {
                        // The value is not in the map already
                        // This must either be a member select of `e` or the entire
                        // constant use is invalid

                        let e_val = match a.value_map.get(&e.node_id) {
                            None => return ControlFlow::Continue(()),
                            Some(v) => v,
                        };

                        let member_val = match e_val {
                            Value::Struct(StructValue { anon_struct, .. })
                            | Value::AnonStruct(anon_struct) => {
                                match anon_struct.members.get(&id.data) {
                                    None => return ControlFlow::Continue(()),
                                    Some(v) => v,
                                }
                            }
                            _ => return ControlFlow::Continue(()),
                        };

                        a.value_map.insert(node.node_id, member_val.clone());
                    }
                    Some(_) => {
                        // If the entire dot expression was already resolved by
                        // a constant_use, the value will already be in this map
                        // No further work is needed
                    }
                }
            }
            ExprKind::Ident(_) => {}
            ExprKind::LiteralBool(v) => {
                a.value_map
                    .insert(node.node_id, Value::Boolean(BooleanValue(*v)));
            }
            ExprKind::LiteralInt(v) => {
                let vi: i128 = if v.starts_with("0x") || v.starts_with("0X") {
                    // Hexadecimal integer literal
                    match i128::from_str_radix(&v[2..], 16) {
                        Ok(v) => v,
                        Err(err) => {
                            SemanticError::InvalidIntValue {
                                loc: node.span(),
                                v: None,
                                msg: format!("failed to parse hexadecimal integral value: {}", err),
                            }
                            .emit();
                            return ControlFlow::Continue(());
                        }
                    }
                } else {
                    // Decimal integer literal
                    match v.parse() {
                        Ok(v) => v,
                        Err(err) => {
                            SemanticError::InvalidIntValue {
                                loc: node.span(),
                                v: None,
                                msg: format!("failed to parse integral value: {}", err),
                            }
                            .emit();
                            return ControlFlow::Continue(());
                        }
                    }
                };

                a.value_map
                    .insert(node.node_id, Value::Integer(IntegerValue(vi)));
            }
            ExprKind::LiteralFloat(v) => {
                let vf: f64 = match v.parse() {
                    Ok(v) => v,
                    Err(err) => {
                        SemanticError::InvalidIntValue {
                            loc: node.span(),
                            v: None,
                            msg: format!("failed to parse floating value: {}", err),
                        }
                        .emit();
                        return ControlFlow::Continue(());
                    }
                };

                a.value_map.insert(
                    node.node_id,
                    Value::Float(FloatValue {
                        value: vf,
                        kind: fpp_ast::FloatKind::F64,
                    }),
                );
            }
            ExprKind::LiteralString(v) => {
                a.value_map
                    .insert(node.node_id, Value::String(StringValue(v.clone())));
            }
            ExprKind::Paren(v) => match a.value_map.get(&v.node_id) {
                None => {}
                Some(v) => {
                    a.value_map.insert(node.node_id, v.clone());
                }
            },
            ExprKind::SizeOf(type_name) => {
                // Finalize the referenced type on demand, then compute its
                // serialized size.
                FinalizeTypeDefs::new().ty(a, type_name);
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
                            a.value_map
                                .insert(node.node_id, Value::Integer(IntegerValue(size)));
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
                        }
                    }
                }
            }
            ExprKind::Struct(struct_expr) => {
                a.value_map.insert(
                    node.node_id,
                    Value::AnonStruct(AnonStructValue {
                        members: HashMap::from_iter(struct_expr.iter().filter_map(|member| {
                            Some((
                                member.name.data.clone(),
                                a.value_map.get(&member.value.node_id)?.clone(),
                            ))
                        })),
                    }),
                );
            }
            ExprKind::Unop { op, e } => {
                // Negation preserves the operand's kind, as `Value.unary_-`
                // does in Scala (`Analysis.neg`)
                if let (Unop::Minus, Some(v)) = (op, a.value_map.get(&e.node_id)) {
                    match v.negate() {
                        Ok(v) => {
                            a.value_map.insert(node.node_id, v);
                        }
                        Err(MathError::InvalidInputs) => {}
                        Err(MathError::Overflow) => {
                            SemanticError::InvalidIntValue {
                                loc: node.span(),
                                v: None,
                                msg: "arithmetic result is too large to represent".to_string(),
                            }
                            .emit();
                        }
                        Err(MathError::DivByZero) => {
                            panic!("unexpected div by zero")
                        }
                        Err(MathError::ShiftOverflow) => {
                            panic!("unexpected shift overflow")
                        }
                    }
                }
            }
        }

        ControlFlow::Continue(())
    }
}

impl<'ast> UseAnalysisPass<'ast, Analysis> for EvalConstantExprs<'ast> {
    fn constant_use(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let symbol = match a.use_def_map.get(&node.node_id) {
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
            _ => return ControlFlow::Continue(()),
        };

        if let Some(value) = a.value_map.get(&symbol.node()) {
            a.value_map.insert(node.node_id, value.clone());
        }

        ControlFlow::Continue(())
    }
}
