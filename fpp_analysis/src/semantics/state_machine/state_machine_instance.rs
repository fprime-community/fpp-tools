use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{Symbol, SymbolInterface, state_machine};
use fpp_ast::{AstNode, DefStateMachine, QueueFull, SpecStateMachineInstance};
use fpp_core::{Node, Span, Spanned};
use std::sync::Arc;

/// An FPP state machine instance
#[derive(Debug, Clone)]
pub struct StateMachineInstance {
    pub node: Arc<SpecStateMachineInstance>,
    /// The state machine this is an instance of
    pub state_machine: Arc<DefStateMachine>,
    pub priority: Option<i128>,
    pub queue_full: QueueFull,
}

impl StateMachineInstance {
    /// Gets the location of the state machine instance
    pub fn get_loc(&self) -> Span {
        self.node.span()
    }

    /// Gets the node ID
    pub fn get_node_id(&self) -> Node {
        self.node.node_id
    }

    /// Gets the name of the state machine instance
    pub fn get_name(&self) -> &str {
        &self.node.name.data
    }

    /// Gets the symbol of the state machine this is an instance of
    pub fn symbol(&self) -> Symbol {
        Symbol::StateMachine(self.state_machine.clone())
    }

    /// Gets the state machine kind
    pub fn get_sm_kind(&self) -> state_machine::Kind {
        state_machine::StateMachine::get_symbol_kind(&self.state_machine)
    }

    /// Creates a state machine instance from a state machine instance specifier.
    ///
    /// Returns `None` if the state machine use did not resolve; CheckUses
    /// reported that.
    pub fn from_spec_state_machine(
        a: &Analysis,
        node: &SpecStateMachineInstance,
    ) -> SemanticResult<Option<StateMachineInstance>> {
        let qid = &node.state_machine;
        let priority = a.get_big_int_value_opt(&node.priority);
        let queue_full = Analysis::get_queue_full(&node.queue_full);
        let state_machine = match a.use_def_map.get(&qid.id()) {
            Some(Symbol::StateMachine(sm)) => sm.clone(),
            Some(symbol) => {
                return Err(SemanticError::InvalidSymbol {
                    symbol_name: symbol.name().data.clone(),
                    loc: qid.span(),
                    msg: "not a state machine symbol".to_string(),
                    def_loc: symbol.node().span(),
                });
            }
            None => return Ok(None),
        };
        Ok(Some(StateMachineInstance {
            node: Arc::new(node.clone()),
            state_machine,
            priority,
            queue_full,
        }))
    }
}
