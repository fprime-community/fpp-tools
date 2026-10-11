use crate::Analysis;
use crate::errors::SemanticError;
use crate::semantics::{Symbol, Type};
use fpp_core::{Diagnostic, Spanned};

/// Check F Prime framework constant values
pub struct CheckFrameworkConstantValues;

impl CheckFrameworkConstantValues {
    /// Checks the value of each framework constant, in the order the model
    /// defines them
    pub fn check(&self, a: &Analysis) {
        for (name, s) in &a.framework_definitions.constant_map {
            if let Err(diagnostic) = check_map_entry(a, name, s) {
                diagnostic.emit();
            }
        }
    }
}

/// The result of checking the value of a framework constant: the diagnostic of
/// a value that does not meet its requirement
type CheckResult = Result<(), Diagnostic>;

/// A requirement on the value of a framework constant, given its name and
/// symbol
type Checker = fn(&Analysis, &str, &Symbol) -> CheckResult;

fn check_map_entry(a: &Analysis, name: &str, s: &Symbol) -> CheckResult {
    match CHECKERS.iter().find(|(n, _)| *n == name) {
        Some((_, checker)) => checker(a, name, s),
        None => Ok(()),
    }
}

/// The value of a framework constant and the location of its value
/// expression. A constant with no value has had its error reported.
fn value_of(a: &Analysis, s: &Symbol) -> Option<(i128, fpp_core::Span)> {
    let Symbol::Constant(def) = s else {
        return None;
    };
    Some((a.get_big_int_value(def.node_id)?, def.value.span()))
}

fn require_nonnegative_integer_constant(a: &Analysis, name: &str, s: &Symbol) -> CheckResult {
    let Some((v, loc)) = value_of(a, s) else {
        return Ok(());
    };
    if v >= 0 {
        Ok(())
    } else {
        Err(SemanticError::InvalidIntValue {
            loc,
            v: Some(v),
            msg: format!(
                "framework definition {} must be a nonnegative integer constant",
                name
            ),
        }
        .into())
    }
}

fn require_positive_integer_constant(a: &Analysis, name: &str, s: &Symbol) -> CheckResult {
    let Some((v, loc)) = value_of(a, s) else {
        return Ok(());
    };
    if v > 0 {
        Ok(())
    } else {
        Err(SemanticError::InvalidIntValue {
            loc,
            v: Some(v),
            msg: format!(
                "framework definition {} must be a positive integer constant",
                name
            ),
        }
        .into())
    }
}

fn require_string_size_constant(a: &Analysis, name: &str, s: &Symbol) -> CheckResult {
    let Some((v, loc)) = value_of(a, s) else {
        return Ok(());
    };
    if Type::is_valid_string_size(v) {
        Ok(())
    } else {
        Err(
            Diagnostic::from(SemanticError::InvalidStringSize { loc, size: v }).note(format!(
                "framework definition {} must be a string size constant",
                name
            )),
        )
    }
}

/// The framework constants whose values have requirements, with the
/// requirements
const CHECKERS: &[(&str, Checker)] = &[
    ("FW_ASSERT_COUNT_MAX", require_nonnegative_integer_constant),
    (
        "FW_CMD_ARG_BUFFER_MAX_SIZE",
        require_positive_integer_constant,
    ),
    ("FW_CMD_STRING_MAX_SIZE", require_string_size_constant),
    ("FW_COM_BUFFER_MAX_SIZE", require_positive_integer_constant),
    ("FW_FILE_BUFFER_MAX_SIZE", require_positive_integer_constant),
    ("FW_FIXED_LENGTH_STRING_SIZE", require_string_size_constant),
    (
        "FW_INTERNAL_INTERFACE_STRING_MAX_SIZE",
        require_string_size_constant,
    ),
    ("FW_LOG_BUFFER_MAX_SIZE", require_positive_integer_constant),
    ("FW_LOG_STRING_MAX_SIZE", require_string_size_constant),
    ("FW_LOG_TEXT_BUFFER_SIZE", require_positive_integer_constant),
    (
        "FW_OBJ_SIMPLE_REG_BUFF_SIZE",
        require_positive_integer_constant,
    ),
    (
        "FW_OBJ_SIMPLE_REG_ENTRIES",
        require_positive_integer_constant,
    ),
    (
        "FW_PARAM_BUFFER_MAX_SIZE",
        require_positive_integer_constant,
    ),
    ("FW_PARAM_STRING_MAX_SIZE", require_string_size_constant),
    (
        "FW_QUEUE_NAME_BUFFER_SIZE",
        require_positive_integer_constant,
    ),
    (
        "FW_QUEUE_SIMPLE_QUEUE_ENTRIES",
        require_positive_integer_constant,
    ),
    (
        "FW_SM_SIGNAL_BUFFER_MAX_SIZE",
        require_positive_integer_constant,
    ),
    (
        "FW_STATEMENT_ARG_BUFFER_MAX_SIZE",
        require_positive_integer_constant,
    ),
    (
        "FW_TASK_NAME_BUFFER_SIZE",
        require_positive_integer_constant,
    ),
    ("FW_TLM_BUFFER_MAX_SIZE", require_positive_integer_constant),
    ("FW_TLM_STRING_MAX_SIZE", require_string_size_constant),
    (
        "Fw.DpCfg.CONTAINER_USER_DATA_SIZE",
        require_positive_integer_constant,
    ),
];
