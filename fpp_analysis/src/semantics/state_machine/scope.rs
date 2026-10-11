use crate::errors::SemanticResult;
use crate::semantics::generic_name_symbol_map::GenericNameSymbolMap;
use crate::semantics::generic_scope::GenericScope;
use crate::semantics::state_machine::{
    StateMachineNameGroup, StateMachineNameGroupMap, StateMachineSymbol,
};

/// A local mapping of unqualified names to state machine symbols
pub type SmNameSymbolMap = GenericNameSymbolMap<StateMachineSymbol>;

/// A collection of name-symbol maps, one for each name group
pub type StateMachineScope = GenericScope<
    StateMachineNameGroup,
    StateMachineSymbol,
    StateMachineNameGroupMap<GenericNameSymbolMap<StateMachineSymbol>>,
>;

/// A stack of scopes
#[derive(Debug, Clone)]
pub struct StateMachineNestedScope {
    scopes: Vec<StateMachineScope>,
}

impl StateMachineNestedScope {
    /// Create an empty NestedScope
    pub fn empty() -> StateMachineNestedScope {
        StateMachineNestedScope {
            scopes: vec![StateMachineScope::new()],
        }
    }

    /// Push a new scope onto the stack
    pub fn push(&mut self, scope: StateMachineScope) {
        self.scopes.push(scope);
    }

    /// Pop a scope off the stack, returning it
    pub fn pop(&mut self) -> StateMachineScope {
        self.scopes.pop().expect("empty scope stack")
    }

    /// Put a name and symbol into the innermost scope
    pub fn put(
        &mut self,
        name_group: StateMachineNameGroup,
        symbol: StateMachineSymbol,
    ) -> SemanticResult {
        self.scopes
            .last_mut()
            .expect("empty scope stack")
            .put(name_group, symbol)
    }

    /// Get a relative or absolute symbol from the nested scope
    pub fn get(
        &self,
        is_absolute: bool,
        name_group: StateMachineNameGroup,
        name: &str,
    ) -> Option<StateMachineSymbol> {
        match is_absolute {
            false => self.get_relative(name_group, name),
            true => self.get_absolute(name_group, name),
        }
    }

    /// Get a relative symbol from the nested scope. Start searching at the
    /// bottom of the stack, working upwards. Return None if the name is not there.
    pub fn get_relative(
        &self,
        name_group: StateMachineNameGroup,
        name: &str,
    ) -> Option<StateMachineSymbol> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name_group, name))
    }

    /// Get an absolute symbol from the nested scope. Start searching at
    /// the top of the stack. Return None if the name is not there
    pub fn get_absolute(
        &self,
        name_group: StateMachineNameGroup,
        name: &str,
    ) -> Option<StateMachineSymbol> {
        self.scopes
            .first()
            .and_then(|scope| scope.get(name_group, name))
    }

    /// Get the innermost nested scope
    pub fn inner_scope(&self) -> &StateMachineScope {
        self.scopes.last().expect("empty scope stack")
    }
}
