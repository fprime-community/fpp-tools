use crate::semantics::{GenericScope, NameGroup, NameGroupMap, NameSymbolMap, Symbol};

/// A collection of name-symbol maps, one for each name group
pub type Scope = GenericScope<NameGroup, Symbol, NameGroupMap<NameSymbolMap>>;
