mod generic_name_symbol_map;
pub use generic_name_symbol_map::*;

mod generic_nested_scope;
pub use generic_nested_scope::*;

use crate::errors::SemanticResult;
use crate::semantics::SymbolInterface;
use fpp_util::EnumMap;
use std::fmt::{Display, Formatter};
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
    pub fn empty() -> Self {
        Self {
            map: M::new(|_| GenericNameSymbolMap::empty()),
            groups: Vec::new(),
            symbol: PhantomData,
        }
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

    /// Get a symbol from the map. Return none if the name is not there.
    pub fn get(&self, name_group: NG, name: &str) -> Option<S> {
        self.map.get(name_group).get(name)
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

/// Each name group that holds a symbol, with the set of its names
impl<NG, S, M> Display for GenericScope<NG, S, M>
where
    NG: Copy + Display,
    S: SymbolInterface,
    M: EnumMap<NG, GenericNameSymbolMap<S>>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("{")?;
        for (i, (ng, nsm)) in self.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            let names: Vec<&str> = nsm.iter().map(|(name, _)| name.as_str()).collect();
            write!(f, "{ng}: {{{}}}", names.join(", "))?;
        }
        f.write_str("}")
    }
}
