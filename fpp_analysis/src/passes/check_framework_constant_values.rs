use crate::Analysis;
use crate::errors::SemanticError;
use crate::semantics::{IntegerValue, Symbol, Type, Value};
use fpp_core::{Diagnostic, Spanned};
use std::sync::Arc;

/// The constraint applied to a framework constant's value.
#[derive(Copy, Clone)]
enum ValueCheck {
    NonNegative,
    Positive,
    StringSize,
}

/// Check F Prime framework constant values.
pub struct CheckFrameworkConstantValues;

impl CheckFrameworkConstantValues {
    pub fn check(&self, a: &Analysis) {
        // Collect first to avoid borrowing `a` while emitting.
        let entries: Vec<(String, Symbol)> = a
            .framework_definitions
            .constant_map
            .iter()
            .map(|(name, sym)| (name.clone(), sym.clone()))
            .collect();

        for (name, symbol) in entries {
            let check = match value_check_for(&name) {
                None => continue,
                Some(c) => c,
            };

            let def = match &symbol {
                Symbol::Constant(def) => def,
                _ => continue,
            };

            let value = match a.value_map.get(&def.node_id) {
                None => continue,
                Some(v) => v,
            };

            let v = match value.convert(&Arc::new(Type::Integer)) {
                Some(Value::Integer(IntegerValue(v))) => v,
                _ => continue,
            };

            let loc = def.value.span();
            match check {
                ValueCheck::NonNegative if v < 0 => SemanticError::InvalidIntValue {
                    loc,
                    v: Some(v),
                    msg: format!(
                        "framework definition {} must be a nonnegative integer constant",
                        name
                    ),
                }
                .emit(),
                ValueCheck::Positive if v <= 0 => SemanticError::InvalidIntValue {
                    loc,
                    v: Some(v),
                    msg: format!(
                        "framework definition {} must be a positive integer constant",
                        name
                    ),
                }
                .emit(),
                ValueCheck::StringSize if !Type::is_valid_string_size(v) => {
                    Diagnostic::from(SemanticError::InvalidStringSize { loc, size: v })
                        .note(format!(
                            "framework definition {} must be a string size constant",
                            name
                        ))
                        .emit()
                }
                _ => {}
            }
        }
    }
}

fn value_check_for(name: &str) -> Option<ValueCheck> {
    Some(match name {
        "FW_ASSERT_COUNT_MAX" => ValueCheck::NonNegative,
        "FW_CMD_STRING_MAX_SIZE"
        | "FW_FIXED_LENGTH_STRING_SIZE"
        | "FW_INTERNAL_INTERFACE_STRING_MAX_SIZE"
        | "FW_LOG_STRING_MAX_SIZE"
        | "FW_PARAM_STRING_MAX_SIZE"
        | "FW_TLM_STRING_MAX_SIZE" => ValueCheck::StringSize,
        "FW_CMD_ARG_BUFFER_MAX_SIZE"
        | "FW_COM_BUFFER_MAX_SIZE"
        | "FW_FILE_BUFFER_MAX_SIZE"
        | "FW_LOG_BUFFER_MAX_SIZE"
        | "FW_LOG_TEXT_BUFFER_SIZE"
        | "FW_OBJ_SIMPLE_REG_BUFF_SIZE"
        | "FW_OBJ_SIMPLE_REG_ENTRIES"
        | "FW_PARAM_BUFFER_MAX_SIZE"
        | "FW_QUEUE_NAME_BUFFER_SIZE"
        | "FW_QUEUE_SIMPLE_QUEUE_ENTRIES"
        | "FW_SM_SIGNAL_BUFFER_MAX_SIZE"
        | "FW_STATEMENT_ARG_BUFFER_MAX_SIZE"
        | "FW_TASK_NAME_BUFFER_SIZE"
        | "FW_TLM_BUFFER_MAX_SIZE"
        | "Fw.DpCfg.CONTAINER_USER_DATA_SIZE" => ValueCheck::Positive,
        _ => return None,
    })
}
