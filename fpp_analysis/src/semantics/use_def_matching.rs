use crate::semantics::{QualifiedName, Symbol, SymbolInterface};
use std::fmt::{Display, Formatter};

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

/// The text names the use and the definition. Their locations are not part of
/// it: a diagnostic shows them as the spans it annotates.
impl Display for UseDefMatching {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "use {} refers to definition {}",
            self.qualified_name,
            self.symbol.name().data
        )
    }
}
