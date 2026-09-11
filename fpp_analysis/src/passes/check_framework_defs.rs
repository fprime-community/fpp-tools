use crate::Analysis;
use crate::errors::SemanticError;
use crate::semantics::{Symbol, SymbolInterface, Type};
use fpp_ast::{
    DefAbsType, DefAliasType, DefArray, DefConstant, DefEnum, DefStruct, IntegerKind, MoveWalkable,
    Node, Visitor,
};
use fpp_core::Spanned;
use std::ops::{ControlFlow, Deref};
use std::sync::Arc;

/// Which validation to apply to a framework type definition.
#[derive(Copy, Clone)]
enum TypeCheck {
    IntegerAlias,
    SignedIntegerAlias,
    UnsignedIntegerAlias,
    Enum,
}

/// Check F Prime framework definitions.
pub struct CheckFrameworkDefs;

impl CheckFrameworkDefs {
    fn is_int(ty: &Arc<Type>) -> bool {
        matches!(
            Type::underlying_type(ty).deref(),
            Type::PrimitiveInt(_) | Type::Integer
        )
    }

    fn signedness_ok(ty: &Arc<Type>, want_signed: bool) -> bool {
        match Type::underlying_type(ty).deref() {
            Type::PrimitiveInt(kind) => is_signed(kind) == want_signed,
            _ => false,
        }
    }

    fn check_module_qualifiers(&self, a: &Analysis, symbol: &Symbol, loc: fpp_core::Span) -> bool {
        let name = a.get_qualified_name(symbol);
        let mut current = symbol.clone();
        while let Some(parent) = a.parent_symbol_map.get(&current) {
            match parent {
                Symbol::Module(_) => current = parent.clone(),
                other => {
                    SemanticError::InvalidQualifier {
                        loc,
                        msg: format!(
                            "framework definition {} must have module qualifiers: {} is not a module symbol",
                            name,
                            a.get_qualified_name(other)
                        ),
                        def_loc: other.name().span(),
                        def_msg: "not a module symbol".to_string(),
                    }
                    .emit();
                    return false;
                }
            }
        }
        true
    }

    fn analyze_type(&self, a: &mut Analysis, symbol: Symbol, loc: fpp_core::Span) {
        let name = a.get_qualified_name(&symbol);
        let check = match type_check_for(&name) {
            None => return,
            Some(c) => c,
        };

        let id = symbol.node();
        let ty = match a.type_map.get(&id) {
            None => return,
            Some(ty) => ty.clone(),
        };

        let ok = match check {
            TypeCheck::IntegerAlias => {
                let ok = Self::is_int(&ty);
                if !ok {
                    SemanticError::InvalidType {
                        loc,
                        msg: format!(
                            "the F Prime framework type {} must be an alias of an integer type",
                            name
                        ),
                    }
                    .emit();
                }
                ok
            }
            TypeCheck::SignedIntegerAlias => {
                let ok = Self::signedness_ok(&ty, true);
                if !ok {
                    SemanticError::InvalidType {
                        loc,
                        msg: format!(
                            "the F Prime framework type {} must be an alias of a signed integer type",
                            name
                        ),
                    }
                    .emit();
                }
                ok
            }
            TypeCheck::UnsignedIntegerAlias => {
                let ok = Self::signedness_ok(&ty, false);
                if !ok {
                    SemanticError::InvalidType {
                        loc,
                        msg: format!(
                            "the F Prime framework type {} must be an alias of an unsigned integer type",
                            name
                        ),
                    }
                    .emit();
                }
                ok
            }
            TypeCheck::Enum => {
                let ok = matches!(ty.deref(), Type::Enum(_));
                if !ok {
                    SemanticError::InvalidType {
                        loc,
                        msg: format!("the F Prime framework type {} must be an enum", name),
                    }
                    .emit();
                }
                ok
            }
        };

        let qualifiers_ok = self.check_module_qualifiers(a, &symbol, loc);
        if ok && qualifiers_ok {
            a.framework_definitions.add_type(name, symbol);
        }
    }
}

fn is_signed(kind: &IntegerKind) -> bool {
    matches!(
        kind,
        IntegerKind::I8 | IntegerKind::I16 | IntegerKind::I32 | IntegerKind::I64
    )
}

fn type_check_for(name: &str) -> Option<TypeCheck> {
    Some(match name {
        "Fw.DpCfg.ProcType" | "Fw.DpState" => TypeCheck::Enum,
        "FwIndexType" | "FwSignedSizeType" => TypeCheck::SignedIntegerAlias,
        "FwSizeType" => TypeCheck::UnsignedIntegerAlias,
        "FwAssertArgType"
        | "FwChanIdType"
        | "FwDpIdType"
        | "FwDpPriorityType"
        | "FwEnumStoreType"
        | "FwEventIdType"
        | "FwOpcodeType"
        | "FwPacketDescriptorType"
        | "FwPriorityType"
        | "FwPrmIdType"
        | "FwQueuePriorityType"
        | "FwSizeStoreType"
        | "FwTimeBaseStoreType"
        | "FwTimeContextStoreType"
        | "FwTlmPacketizeIdType"
        | "FwTraceIdType" => TypeCheck::IntegerAlias,
        _ => return None,
    })
}

fn is_framework_constant(name: &str) -> bool {
    matches!(
        name,
        "FW_ASSERT_COUNT_MAX"
            | "FW_CMD_ARG_BUFFER_MAX_SIZE"
            | "FW_CMD_STRING_MAX_SIZE"
            | "FW_COM_BUFFER_MAX_SIZE"
            | "FW_CONTEXT_DONT_CARE"
            | "FW_FILE_BUFFER_MAX_SIZE"
            | "FW_FIXED_LENGTH_STRING_SIZE"
            | "FW_INTERNAL_INTERFACE_STRING_MAX_SIZE"
            | "FW_LOG_BUFFER_MAX_SIZE"
            | "FW_LOG_STRING_MAX_SIZE"
            | "FW_LOG_TEXT_BUFFER_SIZE"
            | "FW_OBJ_SIMPLE_REG_BUFF_SIZE"
            | "FW_OBJ_SIMPLE_REG_ENTRIES"
            | "FW_PARAM_BUFFER_MAX_SIZE"
            | "FW_PARAM_STRING_MAX_SIZE"
            | "FW_QUEUE_NAME_BUFFER_SIZE"
            | "FW_QUEUE_SIMPLE_QUEUE_ENTRIES"
            | "FW_SERIALIZE_FALSE_VALUE"
            | "FW_SERIALIZE_TRUE_VALUE"
            | "FW_SM_SIGNAL_BUFFER_MAX_SIZE"
            | "FW_STATEMENT_ARG_BUFFER_MAX_SIZE"
            | "FW_TASK_NAME_BUFFER_SIZE"
            | "FW_TLM_BUFFER_MAX_SIZE"
            | "FW_TLM_STRING_MAX_SIZE"
            | "Fw.DpCfg.CONTAINER_USER_DATA_SIZE"
    )
}

impl<'ast> Visitor<'ast> for CheckFrameworkDefs {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        node.walk(a, self)
    }

    fn visit_def_abs_type(
        &self,
        a: &mut Self::State,
        node: &'ast DefAbsType,
    ) -> ControlFlow<Self::Break> {
        self.analyze_type(a, Symbol::AbsType(Arc::new(node.clone())), node.span());
        ControlFlow::Continue(())
    }

    fn visit_def_alias_type(
        &self,
        a: &mut Self::State,
        node: &'ast DefAliasType,
    ) -> ControlFlow<Self::Break> {
        self.analyze_type(a, Symbol::AliasType(Arc::new(node.clone())), node.span());
        ControlFlow::Continue(())
    }

    fn visit_def_array(
        &self,
        a: &mut Self::State,
        node: &'ast DefArray,
    ) -> ControlFlow<Self::Break> {
        self.analyze_type(a, Symbol::ArrayType(Arc::new(node.clone())), node.span());
        ControlFlow::Continue(())
    }

    fn visit_def_enum(&self, a: &mut Self::State, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        self.analyze_type(a, Symbol::EnumType(Arc::new(node.clone())), node.span());
        ControlFlow::Continue(())
    }

    fn visit_def_struct(
        &self,
        a: &mut Self::State,
        node: &'ast DefStruct,
    ) -> ControlFlow<Self::Break> {
        self.analyze_type(a, Symbol::StructType(Arc::new(node.clone())), node.span());
        ControlFlow::Continue(())
    }

    fn visit_def_constant(
        &self,
        a: &mut Self::State,
        node: &'ast DefConstant,
    ) -> ControlFlow<Self::Break> {
        let symbol = Symbol::Constant(Arc::new(node.clone()));
        let name = a.get_qualified_name(&symbol);
        if is_framework_constant(&name) {
            let ok = match a.type_map.get(&node.node_id) {
                Some(ty) => {
                    let ok = Self::is_int(ty);
                    if !ok {
                        SemanticError::InvalidType {
                            loc: node.span(),
                            msg: format!(
                                "the F Prime framework constant {} must have an integer type",
                                name
                            ),
                        }
                        .emit();
                    }
                    ok
                }
                None => false,
            };

            let qualifiers_ok = self.check_module_qualifiers(a, &symbol, node.span());
            if ok && qualifiers_ok {
                a.framework_definitions.add_constant(name, symbol);
            }
        }
        ControlFlow::Continue(())
    }
}
