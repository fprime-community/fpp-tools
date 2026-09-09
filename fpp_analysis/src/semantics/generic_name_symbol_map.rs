use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::SymbolInterface;
use fpp_core::Spanned;
use rustc_hash::FxHashMap as HashMap;

/// A type-generic local mapping of unqualified names to symbols
#[derive(Debug, Clone)]
pub struct GenericNameSymbolMap<S: SymbolInterface>(HashMap<String, S>);

impl<S: SymbolInterface> GenericNameSymbolMap<S> {
    pub fn new() -> Self {
        Self(HashMap::default())
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
                loc: symbol.name().span(),
                prev_loc: prev.node().span(),
            }),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &S)> {
        self.0.iter()
    }
}
