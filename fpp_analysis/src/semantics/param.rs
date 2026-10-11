use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{Type, Value};
use fpp_ast::{Expr, SpecParam};
use fpp_core::{Span, Spanned};
use std::sync::Arc;

/// An FPP parameter
#[derive(Debug, Clone)]
pub struct Param {
    pub node: Arc<SpecParam>,
    pub param_type: Arc<Type>,
    pub default: Option<Value>,
    pub set_opcode: i128,
    pub save_opcode: i128,
    pub is_external: bool,
}

impl Param {
    /// Gets the name of the parameter
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the location of the parameter
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Creates a parameter from a parameter specifier.
    ///
    /// Returns the new default opcode.
    pub fn from_spec_param(
        a: &Analysis,
        node: &SpecParam,
        default_opcode: i128,
    ) -> SemanticResult<(Param, i128)> {
        let param_type = a.get_finalized_type(node.type_name.node_id).unwrap();
        let compute_default_value = |default: &Expr| -> SemanticResult<Option<Value>> {
            if let Some(default_type) = a.type_map.get(&default.node_id) {
                Analysis::convert_types(default.span(), default_type, &param_type)?;
            }
            Ok(a.value_map
                .get(&default.node_id)
                .and_then(|v| v.convert(&param_type)))
        };
        let compute_opcode = |int_opt: Option<i128>, default_opcode: i128| match int_opt {
            Some(i) => Ok((i, default_opcode)),
            None => default_opcode
                .checked_add(1)
                .map(|next| (default_opcode, next))
                .ok_or_else(|| SemanticError::InvalidIntValue {
                    loc: node.span(),
                    v: None,
                    msg: format!(
                        "the default opcode after {} is out of range",
                        default_opcode
                    ),
                }),
        };
        let default = match &node.default {
            Some(default) => compute_default_value(default)?,
            None => None,
        };
        let set_opcode_opt = a.get_nonnegative_big_int_value_opt(&node.set_opcode)?;
        let save_opcode_opt = a.get_nonnegative_big_int_value_opt(&node.save_opcode)?;
        a.check_displayable_type(
            node.type_name.node_id,
            node.type_name.span(),
            "type of parameter is not displayable",
        )?;
        let (set_opcode, default_opcode1) = compute_opcode(set_opcode_opt, default_opcode)?;
        let (save_opcode, default_opcode2) = compute_opcode(save_opcode_opt, default_opcode1)?;
        Ok((
            Param {
                node: Arc::new(node.clone()),
                param_type,
                default,
                set_opcode,
                save_opcode,
                is_external: node.is_external,
            },
            default_opcode2,
        ))
    }
}
