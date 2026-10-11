use crate::semantics::{GenericNameSymbolMap, Symbol};

/// A local mapping of unqualified names to symbols
pub type NameSymbolMap = GenericNameSymbolMap<Symbol>;
