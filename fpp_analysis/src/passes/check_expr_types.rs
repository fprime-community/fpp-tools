use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::UseAnalysisPass;
use crate::analyzers::use_analyzer::UseAnalyzer;
use crate::errors::SemanticError;
use crate::semantics::{
    AnonArrayType, AnonStructType, ArrayType, QualifiedName, StructType, Symbol, SymbolInterface,
    Type, TypeConversionResult,
};
use fpp_ast::{
    DefAliasType, DefArray, DefConstant, DefEnum, DefEnumConstant, DefStruct, Expr, ExprKind,
    FloatKind, IntegerKind, Node, SpecCommand, SpecContainer, SpecEvent, SpecGeneralPortInstance,
    SpecInit, SpecInternalPort, SpecParam, SpecRecord, SpecSpecialPortInstance,
    SpecStateMachineInstance, SpecTlmChannel, SpecTlmPacket, StructTypeMember, TypeName,
    TypeNameKind, Visitable, Visitor,
};
use fpp_core::Spanned;
use rustc_hash::FxHashMap as HashMap;
use std::ops::{ControlFlow, Deref};
use std::sync::Arc;

/// Compute and check expression types, except for array sizes
/// and default values
pub struct CheckExprTypes<'ast> {
    super_: UseAnalyzer<'ast, Self>,
}

impl<'ast> Default for CheckExprTypes<'ast> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> CheckExprTypes<'ast> {
    pub fn new() -> CheckExprTypes<'ast> {
        Self {
            super_: UseAnalyzer::new(),
        }
    }

    fn convert_type(
        &self,
        a: &Analysis,
        expr: &'ast Expr,
        to_ty: &Arc<Type>,
    ) -> TypeConversionResult {
        let from_ty = match a.type_map.get(&expr.node_id) {
            None => return Ok(()),
            Some(ty) => ty,
        };

        Type::convert(from_ty, to_ty)
    }

    fn check_type_is_numerical(&self, a: &Analysis, expr: &'ast Expr) {
        match self.convert_type(a, expr, &Arc::new(Type::Integer)) {
            Ok(_) => {}
            Err(err) => {
                SemanticError::TypeConversion {
                    loc: expr.span(),
                    msg: "cannot convert expression to integer".to_string(),
                    err: Box::new(err),
                }
                .emit();
            }
        }
    }

    /// For shifting operations make sure value is integer or enum
    fn check_integer_or_enum_type(&self, a: &Analysis, expr: &'ast Expr) {
        if let Some(ty) = a.type_map.get(&expr.node_id) {
            let ok = ty.is_int() || matches!(Type::underlying_type(ty).deref(), Type::Enum(_));
            if !ok {
                SemanticError::InvalidType {
                    loc: expr.span(),
                    msg: format!("{} is not an integer or enum type", ty),
                }
                .emit();
            }
        }
    }

    fn check_type_is_numerical_opt(&self, a: &Analysis, expr: &'ast Option<Expr>) {
        match expr {
            None => {}
            Some(expr) => self.check_type_is_numerical(a, expr),
        }
    }

    fn check_expr_matches_node(&self, a: &Analysis, expr: &'ast Expr, node: &fpp_core::Node) {
        if let Some(ty) = a.type_map.get(node) {
            match self.convert_type(a, expr, ty) {
                Ok(_) => {}
                Err(err) => SemanticError::TypeConversion {
                    loc: expr.span(),
                    msg: format!("default value cannot be converted to {}", ty),
                    err: Box::new(err),
                }
                .emit(),
            }
        }
    }

    fn check_expr_opt_matches_node(
        &self,
        a: &Analysis,
        expr: &'ast Option<Expr>,
        node: &fpp_core::Node,
    ) {
        if let Some(expr) = &expr {
            self.check_expr_matches_node(a, expr, node)
        }
    }
}

impl<'ast> Visitor<'ast> for CheckExprTypes<'ast> {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    fn visit_def_array(
        &self,
        a: &mut Self::State,
        node: &'ast DefArray,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::DefArray(node))?;
        self.check_type_is_numerical(a, &node.size);
        self.check_expr_opt_matches_node(a, &node.default, &node.node_id);

        ControlFlow::Continue(())
    }

    fn visit_def_constant(
        &self,
        a: &mut Self::State,
        node: &'ast DefConstant,
    ) -> ControlFlow<Self::Break> {
        if a.type_map.contains_key(&node.node_id) {
            return ControlFlow::Continue(());
        }

        self.super_visit(a, Node::DefConstant(node))?;
        match a.type_map.get(&node.value.node_id) {
            None => {}
            Some(ty) => {
                a.type_map.insert(node.node_id, ty.clone());
            }
        }

        ControlFlow::Continue(())
    }

    fn visit_def_alias_type(
        &self,
        a: &mut Self::State,
        node: &'ast DefAliasType,
    ) -> ControlFlow<Self::Break> {
        if a.type_map.contains_key(&node.node_id) {
            return ControlFlow::Continue(());
        }

        self.super_visit(a, Node::DefAliasType(node))?;
        let ty = a.type_map.get(&node.type_name.node_id).unwrap().clone();
        a.type_map.insert(node.node_id, ty);

        ControlFlow::Continue(())
    }

    fn visit_def_enum(&self, a: &mut Self::State, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::DefEnum(node))?;
        self.check_expr_opt_matches_node(a, &node.default, &node.node_id);

        ControlFlow::Continue(())
    }

    fn visit_def_enum_constant(
        &self,
        a: &mut Self::State,
        node: &'ast DefEnumConstant,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::DefEnumConstant(node))?;

        // Just check that the type of the value expression is convertible to numeric
        // The enum type of the enum constant node is already in the type map
        match &node.value {
            None => {}
            Some(value) => self.check_type_is_numerical(a, value),
        }

        ControlFlow::Continue(())
    }

    fn visit_def_struct(
        &self,
        a: &mut Self::State,
        node: &'ast DefStruct,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::DefStruct(node))?;
        self.check_expr_opt_matches_node(a, &node.default, &node.node_id);

        ControlFlow::Continue(())
    }

    fn visit_expr(&self, a: &mut Self::State, node: &'ast Expr) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::Expr(node))?;

        match &node.kind {
            ExprKind::Array(array) => {
                let first_node = match array.first() {
                    None => {
                        SemanticError::EmptyArray { loc: node.span() }.emit();
                        return ControlFlow::Continue(());
                    }
                    Some(first) => first,
                };

                let first_ty = match a.type_map.get(&first_node.node_id) {
                    Some(first_ty) => first_ty.clone(),
                    // Node doesn't have a type
                    None => return ControlFlow::Continue(()),
                };

                #[allow(clippy::manual_try_fold)]
                let common = array.iter().skip(1).fold(Ok(first_ty), |common, next| {
                    match (common, a.type_map.get(&next.node_id)) {
                        (Ok(common), Some(next_type)) => {
                            match Type::common_type(&common, next_type) {
                                None => Err(SemanticError::InvalidType {
                                    loc: next.span(),
                                    msg: format!(
                                        "cannot find common type between {} and {}",
                                        next_type.as_ref(),
                                        common.as_ref()
                                    ),
                                }),
                                Some(new_ty) => Ok(new_ty),
                            }
                        }
                        (Ok(common), None) => {
                            // Node does not have a type
                            Ok(common)
                        }
                        (Err(err), _) => Err(err),
                    }
                });

                match common {
                    Ok(common) => {
                        // A common type was found across all the elements
                        // Assign the full expression to an array of the proper size
                        a.type_map.insert(
                            node.node_id,
                            Arc::new(Type::AnonArray(AnonArrayType {
                                elt_type: common,
                                size: Some(array.len()),
                            })),
                        );
                    }
                    Err(err) => err.emit(),
                }
            }
            ExprKind::ArraySubscript { e1, e2 } => {
                // Resolve the element type of e1 (and report "not an array type"
                // on e1) before checking that e2 is numeric.
                let arr_ty = match a.type_map.get(&e1.node_id) {
                    None => return ControlFlow::Continue(()),
                    Some(arr_ty) => Type::underlying_type(arr_ty),
                };

                let elt_ty = match arr_ty.deref() {
                    Type::Array(ArrayType { anon_array, .. }) | Type::AnonArray(anon_array) => {
                        anon_array.elt_type.clone()
                    }
                    ty => {
                        SemanticError::InvalidType {
                            loc: e1.span(),
                            msg: format!("{} is not an array type", ty),
                        }
                        .emit();
                        return ControlFlow::Continue(());
                    }
                };

                self.check_type_is_numerical(a, e2);

                a.type_map.insert(node.node_id, elt_ty);
            }
            ExprKind::Binop { left, right, op } => match op {
                fpp_ast::Binop::LShift | fpp_ast::Binop::RShift => {
                    // Shift operation is special, for this we independently check
                    // for both operands to be integer
                    self.check_integer_or_enum_type(a, left);
                    self.check_integer_or_enum_type(a, right);
                    a.type_map.insert(node.node_id, Arc::new(Type::Integer));
                }
                _ => {
                    let ty = match (
                        a.type_map.get(&left.node_id),
                        a.type_map.get(&right.node_id),
                    ) {
                        (Some(lty), Some(rty)) => match Type::common_type(lty, rty) {
                            None => {
                                SemanticError::InvalidType {
                                    loc: node.span(),
                                    msg: format!(
                                        "invalid binary operation between {} and {}",
                                        lty, rty
                                    ),
                                }
                                .emit();
                                return ControlFlow::Continue(());
                            }
                            Some(ty) => ty,
                        },
                        _ => {
                            // One of the sides of the binary operation does not have a type
                            return ControlFlow::Continue(());
                        }
                    };

                    match op {
                        // Addition allows numeric or string operands (string
                        // concatenation). The check is on the common type.
                        fpp_ast::Binop::Add => {
                            if !(ty.is_convertible_to_numeric()
                                || matches!(ty.deref(), Type::String(_)))
                            {
                                SemanticError::InvalidType {
                                    loc: node.span(),
                                    msg: format!(
                                        "cannot convert {} to a numeric or string type",
                                        ty
                                    ),
                                }
                                .emit();
                            }
                        }
                        _ => {
                            // The other operations check the common type once at
                            // the binop node: the type is fine if it is numeric
                            // or convertible to Integer.
                            let numeric = ty.is_numeric()
                                || Type::convert(&ty, &Arc::new(Type::Integer)).is_ok();
                            if !numeric {
                                SemanticError::InvalidType {
                                    loc: node.span(),
                                    msg: format!("cannot convert {} to a numeric type", ty),
                                }
                                .emit();
                            }
                        }
                    }
                    a.type_map.insert(node.node_id, ty);
                }
            },
            ExprKind::SizeOf(type_name) => {
                let ty = a.type_map.get(&type_name.node_id).unwrap();
                if !ty.is_displayable() {
                    SemanticError::InvalidType {
                        loc: node.span(),
                        msg: format!("size of type {} is not known in the model", ty),
                    }
                    .emit();
                }
                a.type_map.insert(node.node_id, Arc::new(Type::Integer));
            }
            ExprKind::Dot { e, id } => {
                if a.type_map.contains_key(&node.node_id) {
                    // Type for this entire dot expression is already resolved
                    // This means it's actually a _use_ of some sort of symbol (constant/enum constant)
                    // We can just use the type that was already resolved
                    return ControlFlow::Continue(());
                }

                // This is some member selection of a struct/anon struct
                let e_ty = match a.type_map.get(&e.node_id) {
                    Some(e_ty) => Type::underlying_type(e_ty),
                    None => return ControlFlow::Continue(()),
                };

                match e_ty.deref() {
                    Type::AnonStruct(anon_struct)
                    | Type::Struct(StructType { anon_struct, .. }) => {
                        match anon_struct.members.get(&id.data) {
                            None => SemanticError::InvalidType {
                                loc: id.span(),
                                msg: format!("{} has no member `{}`", e_ty, id.data),
                            }
                            .emit(),
                            Some(member_ty) => {
                                a.type_map.insert(node.node_id, member_ty.clone());
                            }
                        }
                    }
                    ty => SemanticError::InvalidType {
                        loc: e.span(),
                        msg: format!("{} does not have members", ty),
                    }
                    .emit(),
                }
            }
            ExprKind::Ident(_) => {} // already handled by constant_use
            ExprKind::LiteralBool(_) => {
                a.type_map.insert(node.node_id, Arc::new(Type::Boolean));
            }
            ExprKind::LiteralInt(_) => {
                a.type_map.insert(node.node_id, Arc::new(Type::Integer));
            }
            ExprKind::LiteralFloat(_) => {
                a.type_map
                    .insert(node.node_id, Arc::new(Type::Float(FloatKind::F64)));
            }
            ExprKind::LiteralString(_) => {
                a.type_map
                    .insert(node.node_id, Arc::new(Type::String(None)));
            }
            ExprKind::Paren(e) => {
                if let Some(ty) = a.type_map.get(&e.node_id) {
                    a.type_map.insert(node.node_id, ty.clone());
                }
            }
            ExprKind::Struct(struct_expr) => {
                let mut members_out = HashMap::default();
                let mut member_locs = HashMap::default();

                for member in struct_expr {
                    match a.type_map.get(&member.value.node_id) {
                        Some(member_ty) => {
                            members_out.insert(member.name.data.clone(), member_ty.clone());
                            if let Some(old_member) =
                                member_locs.insert(member.name.data.clone(), member.value.span())
                            {
                                SemanticError::DuplicateStructMember {
                                    name: member.name.data.clone(),
                                    loc: member.span(),
                                    prev_loc: old_member,
                                }
                                .emit();
                            }
                        }
                        _ => return ControlFlow::Continue(()),
                    }
                }

                a.type_map.insert(
                    node.node_id,
                    Arc::new(Type::AnonStruct(AnonStructType {
                        members: members_out,
                    })),
                );
            }
            ExprKind::Unop { e, .. } => {
                self.check_type_is_numerical(a, e);
                if let Some(ty) = a.type_map.get(&e.node_id) {
                    a.type_map.insert(node.node_id, ty.clone());
                }
            }
        }

        ControlFlow::Continue(())
    }

    fn visit_spec_command(
        &self,
        a: &mut Self::State,
        node: &'ast SpecCommand,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecCommand(node))?;
        self.check_type_is_numerical_opt(a, &node.opcode);
        self.check_type_is_numerical_opt(a, &node.priority);

        ControlFlow::Continue(())
    }

    fn visit_spec_container(
        &self,
        a: &mut Self::State,
        node: &'ast SpecContainer,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecContainer(node))?;
        self.check_type_is_numerical_opt(a, &node.id);
        self.check_type_is_numerical_opt(a, &node.default_priority);

        ControlFlow::Continue(())
    }

    fn visit_spec_event(
        &self,
        a: &mut Self::State,
        node: &'ast SpecEvent,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecEvent(node))?;
        self.check_type_is_numerical_opt(a, &node.id);

        // Check that the throttle count/every are valid
        match &node.throttle {
            None => {}
            Some(throttle) => {
                self.check_type_is_numerical(a, &throttle.count);
                if let Some(every) = &throttle.every
                    && let Err(err) = self.convert_type(
                        a,
                        every,
                        &Arc::new(Type::AnonStruct(AnonStructType {
                            members: HashMap::from_iter([
                                (
                                    "seconds".to_string(),
                                    Arc::new(Type::PrimitiveInt(IntegerKind::U32)),
                                ),
                                (
                                    "useconds".to_string(),
                                    Arc::new(Type::PrimitiveInt(IntegerKind::U32)),
                                ),
                            ]),
                        })),
                    )
                {
                    SemanticError::TypeConversion {
                        loc: every.span(),
                        msg: "event throttle every must be convertable to a time interval"
                            .to_string(),
                        err: Box::new(err),
                    }
                    .emit()
                }
            }
        }

        ControlFlow::Continue(())
    }

    fn visit_spec_init(
        &self,
        a: &mut Self::State,
        node: &'ast SpecInit,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecInit(node))?;
        self.check_type_is_numerical(a, &node.phase);
        ControlFlow::Continue(())
    }

    fn visit_spec_internal_port(
        &self,
        a: &mut Self::State,
        node: &'ast SpecInternalPort,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecInternalPort(node))?;
        self.check_type_is_numerical_opt(a, &node.priority);
        ControlFlow::Continue(())
    }

    fn visit_spec_param(
        &self,
        a: &mut Self::State,
        node: &'ast SpecParam,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecParam(node))?;

        self.check_expr_opt_matches_node(a, &node.default, &node.type_name.node_id);
        self.check_type_is_numerical_opt(a, &node.id);
        self.check_type_is_numerical_opt(a, &node.set_opcode);
        self.check_type_is_numerical_opt(a, &node.save_opcode);

        ControlFlow::Continue(())
    }

    fn visit_spec_general_port_instance(
        &self,
        a: &mut Self::State,
        node: &'ast SpecGeneralPortInstance,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecGeneralPortInstance(node))?;

        self.check_type_is_numerical_opt(a, &node.size);
        self.check_type_is_numerical_opt(a, &node.priority);

        ControlFlow::Continue(())
    }

    fn visit_spec_special_port_instance(
        &self,
        a: &mut Self::State,
        node: &'ast SpecSpecialPortInstance,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecSpecialPortInstance(node))?;
        self.check_type_is_numerical_opt(a, &node.priority);
        ControlFlow::Continue(())
    }

    fn visit_spec_record(
        &self,
        a: &mut Self::State,
        node: &'ast SpecRecord,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecRecord(node))?;
        self.check_type_is_numerical_opt(a, &node.id);
        ControlFlow::Continue(())
    }

    fn visit_spec_state_machine_instance(
        &self,
        a: &mut Self::State,
        node: &'ast SpecStateMachineInstance,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecStateMachineInstance(node))?;
        self.check_type_is_numerical_opt(a, &node.priority);
        ControlFlow::Continue(())
    }

    fn visit_spec_tlm_channel(
        &self,
        a: &mut Self::State,
        node: &'ast SpecTlmChannel,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecTlmChannel(node))?;
        self.check_type_is_numerical_opt(a, &node.id);
        for limit in &node.low {
            self.check_type_is_numerical(a, &limit.value);
            self.check_expr_matches_node(a, &limit.value, &node.type_name.node_id)
        }

        for limit in &node.high {
            self.check_type_is_numerical(a, &limit.value);
            self.check_expr_matches_node(a, &limit.value, &node.type_name.node_id)
        }

        ControlFlow::Continue(())
    }

    fn visit_spec_tlm_packet(
        &self,
        a: &mut Self::State,
        node: &'ast SpecTlmPacket,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecTlmPacket(node))?;
        self.check_type_is_numerical_opt(a, &node.id);
        self.check_type_is_numerical(a, &node.group);
        ControlFlow::Continue(())
    }

    fn visit_struct_type_member(
        &self,
        a: &mut Self::State,
        node: &'ast StructTypeMember,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::StructTypeMember(node))?;
        self.check_type_is_numerical_opt(a, &node.size);
        ControlFlow::Continue(())
    }

    fn visit_type_name(
        &self,
        a: &mut Self::State,
        node: &'ast TypeName,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::TypeName(node))?;
        if let TypeNameKind::String(size) = &node.kind {
            self.check_type_is_numerical_opt(a, size);
        }

        ControlFlow::Continue(())
    }
}

impl<'ast> UseAnalysisPass<'ast, Analysis> for CheckExprTypes<'ast> {
    fn constant_use(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let symbol = match a.use_def_map.get(&node.node_id) {
            None => return ControlFlow::Continue(()),
            Some(symbol) => symbol.clone(),
        };

        match &symbol {
            // Constant symbol: visit the constant definition
            // to ensure it has a type
            Symbol::Constant(def) => {
                def.visit(a, self)?;
            }
            // Enum symbol: if this is in scope, then we are in
            // the enum definition, so it already has a type
            Symbol::EnumConstant(_) => {}
            // Invalid use of a symbol in an expression
            _ => {
                SemanticError::InvalidSymbol {
                    symbol_name: symbol.name().data.clone(),
                    msg: "not a constant symbol".to_string(),
                    loc: node.span(),
                    def_loc: symbol.name().span(),
                }
                .emit();
                return ControlFlow::Continue(());
            }
        }

        match a.type_map.get(&symbol.node()) {
            None => {}
            Some(ty) => {
                a.type_map.insert(node.node_id, ty.clone());
            }
        }
        ControlFlow::Continue(())
    }
}
