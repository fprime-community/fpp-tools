use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{UseAnalysisPass, visit_template_arg};
use crate::analyzers::use_analyzer::UseAnalyzer;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    AnonArrayType, AnonStructType, QualifiedName, Symbol, SymbolInterface, Type,
};
use fpp_ast::{
    DefAliasType, DefArray, DefConstant, DefEnum, DefEnumConstant, DefStruct, EventThrottle, Expr,
    ExprKind, FloatKind, IntegerKind, Node, SpecCommand, SpecContainer, SpecEvent,
    SpecGeneralPortInstance, SpecInit, SpecInternalPort, SpecParam, SpecRecord,
    SpecSpecialPortInstance, SpecStateMachineInstance, SpecTlmChannel, SpecTlmPacket,
    StructTypeMember, TypeName, TypeNameKind, Visitable, Visitor,
};
use fpp_core::{Span, Spanned};
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

    /// Check that the type of a node may be converted to a numeric type. A node
    /// with no type failed to type check, and that error is already reported.
    fn convert_node_to_numeric(a: &Analysis, node: &Expr) -> SemanticResult {
        match a.type_map.get(&node.node_id) {
            Some(t) => Self::convert_to_numeric(node.span(), t).map(|_| ()),
            None => Ok(()),
        }
    }

    fn convert_node_to_numeric_opt(a: &Analysis, node_opt: &Option<Expr>) -> SemanticResult {
        match node_opt {
            Some(node) => Self::convert_node_to_numeric(a, node),
            None => Ok(()),
        }
    }

    /// The numeric type that a type converts to: the type itself if it is
    /// numeric, otherwise Integer if the type may be converted to Integer
    fn convert_to_numeric(loc: Span, t: &Arc<Type>) -> SemanticResult<Arc<Type>> {
        if t.is_numeric() {
            Ok(t.clone())
        } else if Type::convert(t, &Arc::new(Type::Integer)).is_ok() {
            Ok(Arc::new(Type::Integer))
        } else {
            Err(SemanticError::InvalidType {
                loc,
                msg: format!("cannot convert {} to a numeric type", t),
                notes: vec![],
            })
        }
    }

    fn convert_to_numeric_or_string(loc: Span, t: &Arc<Type>) -> SemanticResult<Arc<Type>> {
        match (t.deref(), Self::convert_to_numeric(loc, t)) {
            (_, Ok(t1)) => Ok(t1),
            (Type::String(_), _) => Ok(t.clone()),
            _ => Err(SemanticError::InvalidType {
                loc,
                msg: format!("cannot convert {} to a numeric or string type", t),
                notes: vec![],
            }),
        }
    }

    /// For shifting operations make sure value is integer or enum
    fn check_integer_or_enum_type(a: &Analysis, node: &Expr) -> SemanticResult {
        match a.type_map.get(&node.node_id) {
            Some(t) if !(t.is_int() || matches!(t.deref(), Type::Enum(_))) => {
                Err(SemanticError::InvalidType {
                    loc: node.span(),
                    msg: format!("{} is not an integer or enum type", t),
                    notes: vec![],
                })
            }
            _ => Ok(()),
        }
    }

    /// Report the error of a failed check. The analysis goes on with the
    /// remaining checks.
    fn emit_error<T>(result: SemanticResult<T>) -> Option<T> {
        match result {
            Ok(t) => Some(t),
            Err(err) => {
                err.emit();
                None
            }
        }
    }

    /// Check that the type of a default value may be converted to the type
    /// of the node that it is the default for
    fn convert_default(
        a: &Analysis,
        default: &Option<Expr>,
        node: fpp_core::Node,
    ) -> SemanticResult {
        match default {
            Some(default_node) => {
                match (a.type_map.get(&default_node.node_id), a.type_map.get(&node)) {
                    (Some(default_type), Some(t)) => {
                        let loc = default_node.span();
                        Analysis::convert_types(loc, default_type, t).map(|_| ())
                    }
                    // A default with no type failed to type check
                    _ => Ok(()),
                }
            }
            None => Ok(()),
        }
    }

    /// Check the interval of an event throttle
    fn check_throttle_interval(a: &Analysis, every: &Expr) -> SemanticResult {
        match a.type_map.get(&every.node_id) {
            Some(t) => {
                let loc = every.span();
                let interval_type = Arc::new(Type::AnonStruct(AnonStructType {
                    members: vec![
                        (
                            "seconds".to_string(),
                            Arc::new(Type::PrimitiveInt(IntegerKind::U32)),
                        ),
                        (
                            "useconds".to_string(),
                            Arc::new(Type::PrimitiveInt(IntegerKind::U32)),
                        ),
                    ],
                }));
                Analysis::convert_types(loc, t, &interval_type).map(|_| ())
            }
            None => Ok(()),
        }
    }

    /// Check an event throttle
    fn check_throttle(a: &Analysis, throttle: &EventThrottle) -> SemanticResult {
        Self::convert_node_to_numeric(a, &throttle.count)?;
        match &throttle.every {
            Some(every) => Self::check_throttle_interval(a, every),
            None => Ok(()),
        }
    }

    /// Check a telemetry limit expression against the channel type
    fn check_limit_expr(
        a: &Analysis,
        e: &Expr,
        channel_type_node: fpp_core::Node,
    ) -> SemanticResult {
        Self::convert_node_to_numeric(a, e)?;
        match (
            a.type_map.get(&e.node_id),
            a.type_map.get(&channel_type_node),
        ) {
            (Some(limit_type), Some(channel_type)) => {
                let loc = e.span();
                Analysis::convert_types(loc, limit_type, channel_type).map(|_| ())
            }
            _ => Ok(()),
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
        Self::emit_error(Self::convert_node_to_numeric(a, &node.size));
        Self::emit_error(Self::convert_default(a, &node.default, node.node_id));

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
        Self::emit_error(Self::convert_default(a, &node.default, node.node_id));

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
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.value));

        ControlFlow::Continue(())
    }

    fn visit_def_struct(
        &self,
        a: &mut Self::State,
        node: &'ast DefStruct,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::DefStruct(node))?;
        Self::emit_error(Self::convert_default(a, &node.default, node.node_id));

        ControlFlow::Continue(())
    }

    fn visit_expr(&self, a: &mut Self::State, node: &'ast Expr) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::Expr(node))?;

        match &node.kind {
            ExprKind::Array(elts) => {
                // An element with no type failed to type check
                if elts.iter().any(|e| !a.type_map.contains_key(&e.node_id)) {
                    return ControlFlow::Continue(());
                }
                let empty_list_error = SemanticError::EmptyArray { loc: node.span() };
                let ids: Vec<fpp_core::Node> = elts.iter().map(|e| e.node_id).collect();
                match a.common_type(&ids, empty_list_error) {
                    Ok(t) => a.assign_type(
                        node.node_id,
                        Arc::new(Type::AnonArray(AnonArrayType {
                            elt_type: t,
                            size: Some(elts.len()),
                        })),
                    ),
                    Err(err) => {
                        err.emit();
                        return ControlFlow::Break(());
                    }
                }
            }
            ExprKind::ArraySubscript { e1, e2 } => {
                // Resolve the element type of e1 (and report "not an array type"
                // on e1) before checking that e2 is numeric.
                let arr_ty = match a.type_map.get(&e1.node_id) {
                    None => return ControlFlow::Continue(()),
                    Some(arr_ty) => Type::underlying_type(arr_ty),
                };

                let elt_ty = match arr_ty.as_anon_array() {
                    Some(anon_array) => anon_array.elt_type.clone(),
                    None => {
                        let ty = arr_ty.deref();
                        SemanticError::InvalidType {
                            loc: e1.span(),
                            msg: format!("{} is not an array type", ty),
                            notes: vec![],
                        }
                        .emit();
                        return ControlFlow::Break(());
                    }
                };

                Self::emit_error(Self::convert_node_to_numeric(a, e2));

                a.type_map.insert(node.node_id, elt_ty);
            }
            ExprKind::Binop { left, right, op } => match op {
                fpp_ast::Binop::LShift | fpp_ast::Binop::RShift => {
                    // Shift operation is special, for this we independently check
                    // for both operands to be integer
                    let left_ok = Self::emit_error(Self::check_integer_or_enum_type(a, left));
                    let right_ok = Self::emit_error(Self::check_integer_or_enum_type(a, right));
                    if left_ok.is_some() && right_ok.is_some() {
                        a.assign_type(node.node_id, Arc::new(Type::Integer));
                    }
                }
                _ => {
                    // An operand with no type failed to type check
                    if !a.type_map.contains_key(&left.node_id)
                        || !a.type_map.contains_key(&right.node_id)
                    {
                        return ControlFlow::Continue(());
                    }
                    let ty = match a.common_type_of_pair(left.node_id, right.node_id, node.span()) {
                        Ok(ty) => ty,
                        Err(err) => {
                            err.emit();
                            return ControlFlow::Break(());
                        }
                    };

                    // The type of the expression is the numeric type, or for
                    // addition the string type, that the common type converts
                    // to (spec, Type Checking, Arithmetic and Concatenation)
                    let t = match op {
                        fpp_ast::Binop::Add => Self::convert_to_numeric_or_string(node.span(), &ty),
                        _ => Self::convert_to_numeric(node.span(), &ty),
                    };
                    if let Some(t) = Self::emit_error(t) {
                        a.assign_type(node.node_id, t);
                    }
                }
            },
            ExprKind::SizeOf(type_name) => {
                let ty = a.type_map.get(&type_name.node_id).unwrap();
                if !ty.is_displayable() {
                    SemanticError::InvalidType {
                        loc: node.span(),
                        msg: format!("size of type {} is not known in the model", ty),
                        notes: vec![],
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

                match e_ty.as_anon_struct() {
                    Some(anon_struct) => match anon_struct.get_member(&id.data) {
                        None => {
                            SemanticError::InvalidType {
                                loc: id.span(),
                                msg: format!("{} has no member `{}`", e_ty, id.data),
                                notes: vec![],
                            }
                            .emit();
                            return ControlFlow::Break(());
                        }
                        Some(member_ty) => {
                            a.type_map.insert(node.node_id, member_ty.clone());
                        }
                    },
                    None => {
                        SemanticError::InvalidType {
                            loc: e.span(),
                            msg: format!("{} does not have members", e_ty),
                            notes: vec![],
                        }
                        .emit();
                        return ControlFlow::Break(());
                    }
                }
            }
            ExprKind::Ident { .. } => {} // already handled by constant_use
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
                let mut members_out = Vec::default();
                let mut member_locs = HashMap::default();

                for member in struct_expr {
                    match a.type_map.get(&member.value.node_id) {
                        Some(member_ty) => {
                            members_out.push((member.name.data.clone(), member_ty.clone()));
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
                if let Some(t1) = a.type_map.get(&e.node_id)
                    && let Some(t) = Self::emit_error(Self::convert_to_numeric(node.span(), t1))
                {
                    a.assign_type(node.node_id, t);
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
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.opcode));
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.priority));

        ControlFlow::Continue(())
    }

    fn visit_spec_container(
        &self,
        a: &mut Self::State,
        node: &'ast SpecContainer,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecContainer(node))?;
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.id));
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.default_priority));

        ControlFlow::Continue(())
    }

    fn visit_spec_event(
        &self,
        a: &mut Self::State,
        node: &'ast SpecEvent,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecEvent(node))?;
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.id));

        if let Some(throttle) = &node.throttle {
            Self::emit_error(Self::check_throttle(a, throttle));
        }

        ControlFlow::Continue(())
    }

    fn visit_spec_init(
        &self,
        a: &mut Self::State,
        node: &'ast SpecInit,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecInit(node))?;
        Self::emit_error(Self::convert_node_to_numeric(a, &node.phase));
        ControlFlow::Continue(())
    }

    fn visit_spec_internal_port(
        &self,
        a: &mut Self::State,
        node: &'ast SpecInternalPort,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecInternalPort(node))?;
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.priority));
        ControlFlow::Continue(())
    }

    fn visit_spec_param(
        &self,
        a: &mut Self::State,
        node: &'ast SpecParam,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecParam(node))?;

        Self::emit_error(Self::convert_default(
            a,
            &node.default,
            node.type_name.node_id,
        ));
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.id));
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.set_opcode));
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.save_opcode));

        ControlFlow::Continue(())
    }

    fn visit_spec_general_port_instance(
        &self,
        a: &mut Self::State,
        node: &'ast SpecGeneralPortInstance,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecGeneralPortInstance(node))?;

        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.size));
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.priority));

        ControlFlow::Continue(())
    }

    fn visit_spec_special_port_instance(
        &self,
        a: &mut Self::State,
        node: &'ast SpecSpecialPortInstance,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecSpecialPortInstance(node))?;
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.priority));
        ControlFlow::Continue(())
    }

    fn visit_spec_record(
        &self,
        a: &mut Self::State,
        node: &'ast SpecRecord,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecRecord(node))?;
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.id));
        ControlFlow::Continue(())
    }

    fn visit_spec_state_machine_instance(
        &self,
        a: &mut Self::State,
        node: &'ast SpecStateMachineInstance,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecStateMachineInstance(node))?;
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.priority));
        ControlFlow::Continue(())
    }

    fn visit_spec_tlm_channel(
        &self,
        a: &mut Self::State,
        node: &'ast SpecTlmChannel,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecTlmChannel(node))?;
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.id));
        for limit in node.low.iter().chain(&node.high) {
            Self::emit_error(Self::check_limit_expr(
                a,
                &limit.value,
                node.type_name.node_id,
            ));
        }

        ControlFlow::Continue(())
    }

    fn visit_spec_tlm_packet(
        &self,
        a: &mut Self::State,
        node: &'ast SpecTlmPacket,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::SpecTlmPacket(node))?;
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.id));
        Self::emit_error(Self::convert_node_to_numeric(a, &node.group));
        ControlFlow::Continue(())
    }

    fn visit_struct_type_member(
        &self,
        a: &mut Self::State,
        node: &'ast StructTypeMember,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::StructTypeMember(node))?;
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.size));
        ControlFlow::Continue(())
    }

    fn visit_type_name(
        &self,
        a: &mut Self::State,
        node: &'ast TypeName,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::TypeName(node))?;
        if let TypeNameKind::String(size) = &node.kind {
            Self::emit_error(Self::convert_node_to_numeric_opt(a, size));
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
            None => return ControlFlow::Break(()),
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
            // Bound constant parameter: compute the type of the argument bound
            // to it
            Symbol::TemplateConstantArg { value, .. } => {
                if !a.type_map.contains_key(&value.node_id) {
                    self.template_constant_arg(a, &symbol)?;
                }
            }
            // Invalid use of a symbol in an expression
            _ => {
                SemanticError::InvalidSymbol {
                    symbol_name: symbol.name().data.clone(),
                    msg: "not a constant symbol".to_string(),
                    loc: node.span(),
                    def_loc: symbol.node().span(),
                }
                .emit();
                return ControlFlow::Break(());
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

    /// The type of a bound constant parameter is the type declared in the
    /// parameter, so the argument must be convertible to it.
    fn template_constant_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        let Symbol::TemplateConstantArg { param_def, value } = arg else {
            return ControlFlow::Continue(());
        };
        let param_def = param_def.clone();
        let value = value.clone();

        // Compute the type of the argument
        visit_template_arg(self, a, arg)?;

        // Make sure the value can be converted into the type in the template
        let (Some(value_ty), Some(param_ty)) = (
            a.type_map.get(&value.node_id),
            a.type_map.get(&param_def.type_name.node_id),
        ) else {
            return ControlFlow::Continue(());
        };
        match Analysis::convert_types(value.span(), value_ty, param_ty) {
            Ok(ty) => {
                a.assign_type(value.node_id, ty);
                ControlFlow::Continue(())
            }
            Err(err) => {
                err.emit();
                ControlFlow::Break(())
            }
        }
    }

    fn template_type_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        visit_template_arg(self, a, arg)
    }

    fn template_interface_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        visit_template_arg(self, a, arg)
    }
}
