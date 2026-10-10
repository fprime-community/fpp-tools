use crate::semantics::generic_name_symbol_map::GenericNameSymbolMap;
use crate::semantics::generic_nested_scope::GenericNestedScope;
use crate::semantics::generic_scope::GenericScope;
use crate::semantics::{NameGroup, NameGroupMap, Symbol};

/// The thing whose scope is open at one level of a [`NestedScope`]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScopeOwner {
    /// The outermost scope, holding the global definitions at the top level
    Global,
    /// The scope of a definition with a brace-delimited body
    Symbol(Symbol),
    /// The scope holding the bound parameters of a template expansion, keyed by
    /// the node of its expansion specifier. A parameter scope has no owning
    /// definition, so it is not in the symbol scope map.
    TemplateParams(fpp_core::Node),
    /// The scope holding the definitions introduced by a template expansion,
    /// keyed by the node of its expansion specifier
    TemplateExpansion(fpp_core::Node),
}

impl From<Option<Symbol>> for ScopeOwner {
    fn from(value: Option<Symbol>) -> Self {
        match value {
            None => ScopeOwner::Global,
            Some(symbol) => ScopeOwner::Symbol(symbol),
        }
    }
}

/// A stack of scopes
pub type NestedScope = GenericNestedScope<ScopeOwner>;

/// A collection of name-symbol maps, one for each name group
pub type Scope = GenericScope<NameGroup, Symbol, NameGroupMap<GenericNameSymbolMap<Symbol>>>;
