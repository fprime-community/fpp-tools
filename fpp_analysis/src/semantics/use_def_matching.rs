use crate::errors::SymbolUse;
use crate::semantics::{QualifiedName, Symbol, SymbolInterface};
use fpp_core::Spanned;

/// A matching between a use and its definition
#[derive(Debug, Clone)]
pub struct UseDefMatching {
    /// The node Identifier corresponding to the use
    pub node: fpp_core::Node,
    /// The qualified name appearing in the use
    pub qualified_name: QualifiedName,
    /// The symbol corresponding to the definition
    pub symbol: Symbol,
}

impl From<&UseDefMatching> for SymbolUse {
    fn from(value: &UseDefMatching) -> Self {
        SymbolUse {
            def_loc: value.symbol.name().span(),
            use_loc: value.node.span(),
        }
    }
}
