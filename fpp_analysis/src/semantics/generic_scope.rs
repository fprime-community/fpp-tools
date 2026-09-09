use crate::errors::SemanticResult;
use crate::semantics::SymbolInterface;
use crate::semantics::generic_name_symbol_map::GenericNameSymbolMap;
use fpp_util::EnumMap;
use std::marker::PhantomData;

/// A generic collection of name-symbol maps, one for each name group
#[derive(Debug, Clone, Copy)]
pub struct GenericScope<NG, S: SymbolInterface, M: EnumMap<NG, GenericNameSymbolMap<S>>>(
    M,
    PhantomData<NG>,
    PhantomData<S>,
);

impl<NG, S: SymbolInterface, M: EnumMap<NG, GenericNameSymbolMap<S>>> GenericScope<NG, S, M> {
    /// Construct a new scope
    pub fn new() -> Self {
        Self(
            M::new(|_| GenericNameSymbolMap::new()),
            Default::default(),
            Default::default(),
        )
    }

    /// Look up a symbol in this scope
    pub fn get(&self, name_group: NG, name: &str) -> Option<S> {
        self.0.get(name_group).get(name)
    }

    /// Put a name and symbol into the map.
    pub fn put(&mut self, name_group: NG, symbol: S) -> SemanticResult {
        self.0.get_mut(name_group).put(symbol)
    }

    /// Get the name-symbol map for a name group
    pub fn get_group(&self, name_group: NG) -> &GenericNameSymbolMap<S> {
        self.0.get(name_group)
    }
}
