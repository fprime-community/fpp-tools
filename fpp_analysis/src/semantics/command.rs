use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use fpp_ast::{FormalParam, InputPortKind, QueueFull, SpecCommand, SpecParam};
use fpp_core::{Span, Spanned};
use std::sync::Arc;

/// An FPP command
#[derive(Debug, Clone)]
pub enum Command {
    /// A non-parameter command
    NonParam {
        node: Arc<SpecCommand>,
        kind: NonParamKind,
    },
    /// A parameter command
    Param {
        node: Arc<SpecParam>,
        kind: ParamKind,
    },
}

/// A command opcode
pub type CommandOpcode = i128;

/// The kind of a non-parameter command
#[derive(Debug, Clone)]
pub enum NonParamKind {
    Async {
        priority: Option<i128>,
        queue_full: QueueFull,
    },
    Guarded,
    Sync,
}

/// The kind of a parameter command
#[derive(Debug, Clone)]
pub enum ParamKind {
    Save,
    Set,
}

impl Command {
    /// Gets the location of the command
    pub fn get_loc(&self) -> Span {
        match self {
            Command::NonParam { node, .. } => node.span(),
            Command::Param { node, .. } => node.span(),
        }
    }

    /// Gets the name of the command
    pub fn get_name(&self) -> String {
        match self {
            Command::NonParam { node, .. } => node.name.data.clone(),
            Command::Param { node, kind } => {
                let param_name = node.name.data.to_uppercase();
                match kind {
                    ParamKind::Save => format!("{}_PRM_SAVE", param_name),
                    ParamKind::Set => format!("{}_PRM_SET", param_name),
                }
            }
        }
    }

    /// Whether this is an async command. Rust-only, for the Python bindings.
    pub fn is_async(&self) -> bool {
        matches!(
            self,
            Command::NonParam {
                kind: NonParamKind::Async { .. },
                ..
            }
        )
    }

    /// Creates a command from a command specifier
    pub fn from_spec_command(a: &Analysis, node: &SpecCommand) -> SemanticResult<Command> {
        let loc = node.span();
        let check_ref_params = |params: &[FormalParam]| {
            if Analysis::get_num_ref_params(params) != 0 {
                Err(SemanticError::InvalidCommand {
                    loc,
                    msg: "command may not have ref parameters".to_string(),
                })
            } else {
                Ok(())
            }
        };
        if !matches!(node.kind, InputPortKind::Async) {
            if let Some(priority) = &node.priority {
                return Err(SemanticError::InvalidPriority {
                    loc: priority.span(),
                });
            }
            if let Some(queue_full) = &node.queue_full {
                return Err(SemanticError::InvalidQueueFull {
                    loc: queue_full.span(),
                });
            }
        }
        let priority = a.get_big_int_value_opt(&node.priority);
        Analysis::check_for_duplicate_parameter(&node.params)?;
        check_ref_params(&node.params)?;
        a.check_displayable_params(&node.params, "type of command parameter is not displayable")?;
        let kind = match node.kind {
            InputPortKind::Async => NonParamKind::Async {
                priority,
                queue_full: Analysis::get_specified_queue_full(&node.queue_full),
            },
            InputPortKind::Guarded => NonParamKind::Guarded,
            InputPortKind::Sync => NonParamKind::Sync,
        };
        Ok(Command::NonParam {
            node: Arc::new(node.clone()),
            kind,
        })
    }
}
