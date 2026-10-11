use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::SymbolInterface;
use indexmap::IndexMap;
use rustc_hash::FxBuildHasher;

/// A type-generic local mapping of unqualified names to symbols
///
/// The names iterate in the order they were put into the map.
#[derive(Debug, Clone)]
pub struct GenericNameSymbolMap<S: SymbolInterface>(IndexMap<String, S, FxBuildHasher>);

impl<S: SymbolInterface> GenericNameSymbolMap<S> {
    pub fn new() -> Self {
        Self(IndexMap::default())
    }

    /// Get a symbol from the map. Return none if the name is not there.
    pub fn get(&self, name: &str) -> Option<S> {
        self.0.get(name).cloned()
    }

    /// Put a name and symbol into the map.
    pub fn put(&mut self, symbol: S) -> SemanticResult {
        let name = symbol.name().data.as_str();
        match self.0.get(name) {
            None => {
                self.0.insert(name.to_string(), symbol);
                Ok(())
            }
            Some(prev) => Err(SemanticError::RedefinedSymbol {
                name: name.to_string(),
                loc: symbol.get_loc(),
                prev_loc: prev.get_loc(),
            }),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &S)> {
        self.0.iter()
    }
}
