use crate::semantics::generic_name_symbol_map::GenericNameSymbolMap;
use crate::semantics::generic_nested_scope::GenericNestedScope;
use crate::semantics::generic_scope::GenericScope;
use crate::semantics::{NameGroup, NameGroupMap, Symbol};

/// A stack of scopes
pub type NestedScope =
    GenericNestedScope<NameGroup, Symbol, NameGroupMap<GenericNameSymbolMap<Symbol>>>;

/// A collection of name-symbol maps, one for each name group
pub type Scope = GenericScope<NameGroup, Symbol, NameGroupMap<GenericNameSymbolMap<Symbol>>>;
