use crate::errors::SemanticResult;
use crate::semantics::SymbolInterface;
use crate::semantics::generic_name_symbol_map::GenericNameSymbolMap;
use fpp_util::EnumMap;
use std::marker::PhantomData;

/// A generic collection of name-symbol maps, one for each name group
#[derive(Debug, Clone)]
pub struct GenericScope<NG, S: SymbolInterface, M: EnumMap<NG, GenericNameSymbolMap<S>>> {
    /// The map
    map: M,
    /// The name groups that hold a symbol, in the order they received their
    /// first symbol
    groups: Vec<NG>,
    symbol: PhantomData<S>,
}

impl<NG: Copy, S: SymbolInterface, M: EnumMap<NG, GenericNameSymbolMap<S>>> GenericScope<NG, S, M> {
    /// Create an empty scope
    pub fn new() -> Self {
        Self {
            map: M::new(|_| GenericNameSymbolMap::empty()),
            groups: Vec::new(),
            symbol: PhantomData,
        }
    }

    /// Get a symbol from the map. Return none if the name is not there.
    pub fn get(&self, name_group: NG, name: &str) -> Option<S> {
        self.map.get(name_group).get(name)
    }

    /// Put a name and symbol into the map.
    pub fn put(&mut self, name_group: NG, symbol: S) -> SemanticResult {
        let nsm = self.map.get_mut(name_group);
        let is_new_group = nsm.is_empty();
        nsm.put(symbol)?;
        if is_new_group {
            self.groups.push(name_group);
        }
        Ok(())
    }

    /// Get the name-symbol map for a name group
    pub fn get_group(&self, name_group: NG) -> &GenericNameSymbolMap<S> {
        self.map.get(name_group)
    }

    /// The name groups that hold a symbol, each with its name-symbol map, in
    /// the order the groups received their first symbol
    pub fn iter(&self) -> impl Iterator<Item = (NG, &GenericNameSymbolMap<S>)> {
        self.groups.iter().map(|ng| (*ng, self.map.get(*ng)))
    }
}
