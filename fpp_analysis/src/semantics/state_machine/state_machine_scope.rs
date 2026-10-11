use crate::semantics::generic_scope::GenericScope;
use crate::semantics::state_machine::{
    SmNameSymbolMap, StateMachineNameGroup, StateMachineNameGroupMap, StateMachineSymbol,
};

/// A collection of name-symbol maps, one for each name group
pub type StateMachineScope = GenericScope<
    StateMachineNameGroup,
    StateMachineSymbol,
    StateMachineNameGroupMap<SmNameSymbolMap>,
>;
