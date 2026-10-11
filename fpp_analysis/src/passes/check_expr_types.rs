use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::basic_use_analyzer::{UseAnalysisPass, visit_template_arg};
use crate::analyzers::use_analyzer::UseAnalyzer;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    AnonArrayType, AnonStructType, QualifiedName, Symbol, SymbolInterface, Type,
};
use fpp_ast::{
    Binop, Connection, DefAliasType, DefArray, DefComponentInstance, DefConstant, DefEnum,
    DefEnumConstant, DefStruct, EventThrottle, Expr, ExprKind, FloatKind, Ident, IntegerKind, Node,
    SpecCommand, SpecContainer, SpecEvent, SpecGeneralPortInstance, SpecInit, SpecInternalPort,
    SpecParam, SpecRecord, SpecSpecialPortInstance, SpecStateMachineInstance, SpecTlmChannel,
    SpecTlmPacket, StructExprMember, StructTypeMember, TypeName, TypeNameKind, Visitable, Visitor,
};
use fpp_core::{Span, Spanned};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::cell::RefCell;
use std::ops::{ControlFlow, Deref};
use std::sync::Arc;

/// Compute and check expression types, except for array sizes
/// and default values
pub struct CheckExprTypes<'ast> {
    super_: UseAnalyzer<'ast, Self>,
    /// The constant definitions and template constant arguments visited so
    /// far. One whose value fails to type check gets no type, so the type map
    /// does not record that it was visited.
    visited: RefCell<HashSet<fpp_core::Node>>,
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
            visited: RefCell::default(),
        }
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
        self.constant_use_symbol(a, node, symbol)
    }

    fn implied_constant_use(
        &self,
        a: &mut Analysis,
        node: &Expr,
        _: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        // An implied use that does not resolve has no use-def entry
        match a.use_def_map.get(&node.node_id).cloned() {
            Some(symbol) => self.constant_use_symbol(a, node, symbol),
            None => ControlFlow::Continue(()),
        }
    }

    /// The type of a bound constant parameter is the type declared in the
    /// parameter, so the argument must be convertible to it.
    fn template_constant_arg(&self, a: &mut Analysis, arg: &Symbol) -> ControlFlow<Self::Break> {
        let Symbol::TemplateConstantArg { param_def, value } = arg else {
            return ControlFlow::Continue(());
        };
        let param_def = param_def.clone();
        let value = value.clone();
        if !self.visited.borrow_mut().insert(value.node_id) {
            return ControlFlow::Continue(());
        }

        // Visit the literal value
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

    /// The expressions of a component instance must have numeric types (spec,
    /// Component Instance Definitions)
    fn visit_def_component_instance(
        &self,
        a: &mut Self::State,
        node: &'ast DefComponentInstance,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::DefComponentInstance(node))?;
        Self::emit_error(Self::convert_node_to_numeric(a, &node.base_id));
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.queue_size));
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.stack_size));
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.priority));
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.cpu));

        ControlFlow::Continue(())
    }

    fn visit_def_constant(
        &self,
        a: &mut Self::State,
        node: &'ast DefConstant,
    ) -> ControlFlow<Self::Break> {
        if a.type_map.contains_key(&node.node_id) || !self.visited.borrow_mut().insert(node.node_id)
        {
            return ControlFlow::Continue(());
        }

        self.super_visit(a, Node::DefConstant(node))?;
        if let Some(t) = a.type_map.get(&node.value.node_id).cloned() {
            a.assign_type(node.node_id, t);
        }

        ControlFlow::Continue(())
    }

    fn visit_def_alias_type(
        &self,
        a: &mut Self::State,
        node: &'ast DefAliasType,
    ) -> ControlFlow<Self::Break> {
        // CheckTypeUses has given the definition its type, but the type name
        // is still visited, so that the size of a string type is checked
        self.super_visit(a, Node::DefAliasType(node))?;
        if !a.type_map.contains_key(&node.node_id)
            && let Some(t) = a.type_map.get(&node.type_name.node_id).cloned()
        {
            a.assign_type(node.node_id, t);
        }

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
        match &node.kind {
            ExprKind::Array(elts) => self.expr_array_node(a, node, elts),
            ExprKind::ArraySubscript { e1, e2 } => self.expr_array_subscript_node(a, node, e1, e2),
            ExprKind::Binop { left, op, right } => self.expr_binop_node(a, node, left, op, right),
            ExprKind::Dot { e, id } => self.expr_dot_node(a, node, e, id),
            // An identifier is a constant use, typed by constant_use
            ExprKind::Ident { .. } => self.super_visit(a, Node::Expr(node)),
            ExprKind::LiteralBool(_) => self.expr_literal_bool_node(a, node),
            ExprKind::LiteralFloat(_) => self.expr_literal_float_node(a, node),
            ExprKind::LiteralInt(_) => self.expr_literal_int_node(a, node),
            ExprKind::LiteralString(_) => self.expr_literal_string_node(a, node),
            ExprKind::Paren(e) => self.expr_paren_node(a, node, e),
            ExprKind::SizeOf(type_name) => self.expr_size_of_node(a, node, type_name),
            ExprKind::Struct(members) => self.expr_struct_node(a, node, members),
            ExprKind::Unop { e, .. } => self.expr_unop_node(a, node, e),
        }
    }

    /// The port numbers of a connection must have numeric types (spec,
    /// Connection Graph Specifiers)
    fn visit_connection(
        &self,
        a: &mut Self::State,
        node: &'ast Connection,
    ) -> ControlFlow<Self::Break> {
        self.super_visit(a, Node::Connection(node))?;
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.from_index));
        Self::emit_error(Self::convert_node_to_numeric_opt(a, &node.to_index));

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
            Self::check_throttle(a, throttle);
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

impl<'ast> CheckExprTypes<'ast> {
    /// Type a constant use, given the symbol that it refers to
    fn constant_use_symbol(
        &self,
        a: &mut Analysis,
        node: &Expr,
        symbol: Symbol,
    ) -> ControlFlow<()> {
        match &symbol {
            // Constant symbol: visit the constant definition
            // to ensure it has a type
            Symbol::Constant(def) => {
                def.visit(a, self)?;
            }
            // Enum symbol: if this is in scope, then we are in
            // the enum definition, so it already has a type
            Symbol::EnumConstant(_) => {}
            // Template constant argument symbol
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

        if let Some(t) = a.type_map.get(&symbol.node()).cloned() {
            a.assign_type(node.node_id, t);
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
        // An element with no type failed to type check
        if elts.iter().any(|e| !a.type_map.contains_key(&e.node_id)) {
            return ControlFlow::Continue(());
        }
        let empty_list_error = SemanticError::EmptyArray { loc: node.span() };
        let ids: Vec<fpp_core::Node> = elts.iter().map(|e| e.node_id).collect();
        match a.common_type(&ids, empty_list_error) {
            Ok(t) => {
                a.assign_type(
                    node.node_id,
                    Arc::new(Type::AnonArray(AnonArrayType {
                        size: Some(elts.len()),
                        elt_type: t,
                    })),
                );
                ControlFlow::Continue(())
            }
            Err(err) => {
                err.emit();
                ControlFlow::Break(())
            }
        }
    }

    fn expr_array_subscript_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        e1: &'ast Expr,
        e2: &'ast Expr,
    ) -> ControlFlow<()> {
        // Visit e1 and check that it is an array before visiting e2
        e1.visit(a, self)?;
        let Some(t1) = a.type_map.get(&e1.node_id) else {
            return ControlFlow::Continue(());
        };
        let elt_type = match Type::underlying_type(t1).as_anon_array() {
            Some(anon_array) => anon_array.elt_type.clone(),
            None => {
                SemanticError::InvalidType {
                    loc: e1.span(),
                    msg: format!("{} is not an array type", Type::underlying_type(t1)),
                    notes: vec![],
                }
                .emit();
                return ControlFlow::Break(());
            }
        };

        e2.visit(a, self)?;
        if !a.type_map.contains_key(&e2.node_id) {
            return ControlFlow::Continue(());
        }
        if Self::emit_error(Self::convert_node_to_numeric(a, e2)).is_some() {
            a.assign_type(node.node_id, elt_type);
        }
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
        let loc = node.span();
        self.super_visit(a, Node::Expr(node))?;
        // An operand with no type failed to type check
        if !a.type_map.contains_key(&e1.node_id) || !a.type_map.contains_key(&e2.node_id) {
            return ControlFlow::Continue(());
        }
        let t = match op {
            Binop::LShift | Binop::RShift => {
                // Shift operation is special, for this we independently check
                // for both operands to be integer
                let ok1 = Self::emit_error(Self::check_integer_or_enum_type(a, e1));
                let ok2 = Self::emit_error(Self::check_integer_or_enum_type(a, e2));
                match (ok1, ok2) {
                    (Some(()), Some(())) => Some(Arc::new(Type::Integer)),
                    _ => None,
                }
            }
            _ => {
                let t = match a.common_type_of_pair(e1.node_id, e2.node_id, loc) {
                    Ok(t) => t,
                    Err(err) => {
                        err.emit();
                        return ControlFlow::Break(());
                    }
                };
                // The type of the expression is the numeric type, or for
                // addition the string type, that the common type converts to
                // (spec, Type Checking, Arithmetic and Concatenation)
                Self::emit_error(match op {
                    Binop::Add => Self::convert_to_numeric_or_string(loc, &t),
                    _ => Self::convert_to_numeric(loc, &t),
                })
            }
        };
        if let Some(t) = t {
            a.assign_type(node.node_id, t);
        }
        ControlFlow::Continue(())
    }

    fn expr_literal_bool_node(&self, a: &mut Analysis, node: &'ast Expr) -> ControlFlow<()> {
        a.assign_type(node.node_id, Arc::new(Type::Boolean));
        ControlFlow::Continue(())
    }

    fn expr_literal_float_node(&self, a: &mut Analysis, node: &'ast Expr) -> ControlFlow<()> {
        a.assign_type(node.node_id, Arc::new(Type::Float(FloatKind::F64)));
        ControlFlow::Continue(())
    }

    fn expr_literal_int_node(&self, a: &mut Analysis, node: &'ast Expr) -> ControlFlow<()> {
        a.assign_type(node.node_id, Arc::new(Type::Integer));
        ControlFlow::Continue(())
    }

    fn expr_literal_string_node(&self, a: &mut Analysis, node: &'ast Expr) -> ControlFlow<()> {
        a.assign_type(node.node_id, Arc::new(Type::String(None)));
        ControlFlow::Continue(())
    }

    fn expr_paren_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        e: &'ast Expr,
    ) -> ControlFlow<()> {
        self.super_visit(a, Node::Expr(node))?;
        if let Some(t) = a.type_map.get(&e.node_id).cloned() {
            a.assign_type(node.node_id, t);
        }
        ControlFlow::Continue(())
    }

    fn expr_size_of_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        type_name: &'ast TypeName,
    ) -> ControlFlow<()> {
        if let Some(t) = a.type_map.get(&type_name.node_id) {
            let msg = format!("size of type {} is not known in the model", t);
            let result = a.check_displayable_type(type_name.node_id, type_name.span(), &msg);
            if Self::emit_error(result).is_none() {
                return ControlFlow::Continue(());
            }
        }
        self.super_visit(a, Node::Expr(node))?;
        a.assign_type(node.node_id, Arc::new(Type::Integer));
        ControlFlow::Continue(())
    }

    fn expr_struct_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        members: &'ast [StructExprMember],
    ) -> ControlFlow<()> {
        let no_duplicates = Self::emit_error(Self::check_for_duplicate_struct_member(members));
        self.super_visit(a, Node::Expr(node))?;
        if no_duplicates.is_none() {
            return ControlFlow::Continue(());
        }
        let mut struct_members = Vec::with_capacity(members.len());
        for member in members {
            match a.type_map.get(&member.value.node_id) {
                Some(t) => struct_members.push((member.name.data.clone(), t.clone())),
                // A member with no type failed to type check
                None => return ControlFlow::Continue(()),
            }
        }
        a.assign_type(
            node.node_id,
            Arc::new(Type::AnonStruct(AnonStructType {
                members: struct_members,
            })),
        );
        ControlFlow::Continue(())
    }

    fn expr_unop_node(&self, a: &mut Analysis, node: &'ast Expr, e: &'ast Expr) -> ControlFlow<()> {
        let loc = node.span();
        self.super_visit(a, Node::Expr(node))?;
        if let Some(t1) = a.type_map.get(&e.node_id)
            && let Some(t) = Self::emit_error(Self::convert_to_numeric(loc, t1))
        {
            a.assign_type(node.node_id, t);
        }
        ControlFlow::Continue(())
    }

    fn expr_dot_node(
        &self,
        a: &mut Analysis,
        node: &'ast Expr,
        e: &'ast Expr,
        id: &'ast Ident,
    ) -> ControlFlow<()> {
        self.super_visit(a, Node::Expr(node))?;

        // Type for this entire dot expression is already resolved
        // This means it's actually a _use_ of some sort of symbol (constant/enum constant)
        // We can just use the type that was already resolved
        if a.type_map.contains_key(&node.node_id) {
            return ControlFlow::Continue(());
        }

        match a.use_def_map.get(&node.node_id) {
            // A use of a constant symbol whose definition failed to type check
            Some(
                Symbol::Constant(_) | Symbol::EnumConstant(_) | Symbol::TemplateConstantArg { .. },
            ) => {
                return ControlFlow::Continue(());
            }
            // A dot expression that names a symbol of another kind, such as a
            // type with a scope, is not a constant use
            Some(symbol) => {
                SemanticError::InvalidSymbol {
                    symbol_name: symbol.name().data.clone(),
                    msg: "not a constant symbol".to_string(),
                    loc: node.span(),
                    def_loc: symbol.node().span(),
                }
                .emit();
                return ControlFlow::Break(());
            }
            None => {}
        }

        // The type was not resolved, this is some sort of member selection
        // Look at the left-hand side of the dot expression to determine the type
        let Some(e_type) = a.type_map.get(&e.node_id) else {
            return ControlFlow::Continue(());
        };
        let result = match Type::underlying_type(e_type).deref() {
            Type::Struct(struct_type) => match struct_type.anon_struct.get_member(&id.data) {
                Some(t) => Ok(t.clone()),
                None => Err(SemanticError::InvalidStructMember {
                    member_name: id.data.clone(),
                    loc: id.span(),
                    struct_type_name: struct_type.node.name.data.clone(),
                    def_loc: struct_type.node.span(),
                }),
            },
            Type::AnonStruct(anon_struct) => match anon_struct.get_member(&id.data) {
                Some(t) => Ok(t.clone()),
                None => Err(SemanticError::InvalidAnonStructMember {
                    member_name: id.data.clone(),
                    loc: id.span(),
                    def_loc: a.use_def_map.get(&e.node_id).map(|s| s.node().span()),
                }),
            },
            t => Err(SemanticError::InvalidTypeForMemberSelection {
                member_name: id.data.clone(),
                loc: id.span(),
                type_name: t.to_string(),
            }),
        };
        match result {
            Ok(t) => {
                a.assign_type(node.node_id, t);
                ControlFlow::Continue(())
            }
            Err(err) => {
                err.emit();
                ControlFlow::Break(())
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
    ///
    /// The count and the interval are separate expressions, so each one is
    /// checked even when the other fails.
    fn check_throttle(a: &Analysis, throttle: &EventThrottle) {
        Self::emit_error(Self::convert_node_to_numeric(a, &throttle.count));
        if let Some(every) = &throttle.every {
            Self::emit_error(Self::check_throttle_interval(a, every));
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

    /// Check for duplicate struct member
    fn check_for_duplicate_struct_member(members: &[StructExprMember]) -> SemanticResult {
        let mut prev_locs: HashMap<&str, Span> = HashMap::default();
        for member in members {
            if let Some(prev_loc) = prev_locs.insert(&member.name.data, member.span()) {
                return Err(SemanticError::DuplicateStructMember {
                    name: member.name.data.clone(),
                    loc: member.span(),
                    prev_loc,
                });
            }
        }
        Ok(())
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
}
