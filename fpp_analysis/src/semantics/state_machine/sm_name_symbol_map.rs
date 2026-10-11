use crate::semantics::generic_name_symbol_map::GenericNameSymbolMap;
use crate::semantics::state_machine::StateMachineSymbol;

/// A local mapping of unqualified names to state machine symbols
pub type SmNameSymbolMap = GenericNameSymbolMap<StateMachineSymbol>;
