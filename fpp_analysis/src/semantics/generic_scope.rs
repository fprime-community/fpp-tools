use crate::errors::SemanticResult;
use crate::semantics::SymbolInterface;
use crate::semantics::generic_name_symbol_map::GenericNameSymbolMap;
use fpp_util::EnumMap;
use std::marker::PhantomData;

/// A generic collection of name-symbol maps, one for each name group
#[derive(Debug, Clone)]
pub struct GenericScope<NG, S: SymbolInterface, M: EnumMap<NG, GenericNameSymbolMap<S>>>(
    M,
    PhantomData<NG>,
    PhantomData<S>,
);

impl<NG, S: SymbolInterface, M: EnumMap<NG, GenericNameSymbolMap<S>>> GenericScope<NG, S, M> {
    /// Create an empty scope
    pub fn new() -> Self {
        Self(
            M::new(|_| GenericNameSymbolMap::empty()),
            Default::default(),
            Default::default(),
        )
    }

    /// Get a symbol from the map. Return none if the name is not there.
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
