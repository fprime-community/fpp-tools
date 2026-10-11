use std::sync::Arc;

use crate::{Analysis, semantics::Symbol};

/// A type definition that [`crate::passes::EnterSymbols`] interns into a
/// [`Symbol`]
pub trait InternedDef: fpp_ast::AstNode + Clone {
    /// The definition held by a symbol of this kind of definition
    fn from_symbol(symbol: &Symbol) -> Option<&Arc<Self>>;
}

macro_rules! impl_interned_def {
    ($($ty:ident => $variant:ident),* $(,)?) => {
        $(impl InternedDef for fpp_ast::$ty {
            fn from_symbol(symbol: &Symbol) -> Option<&Arc<Self>> {
                match symbol {
                    Symbol::$variant(def) => Some(def),
                    _ => None,
                }
            }
        })*
    };
}

impl_interned_def! {
    DefAbsType => AbsType,
    DefAliasType => AliasType,
    DefArray => ArrayType,
    DefEnum => EnumType,
    DefStruct => StructType,
}

impl Analysis {
    /// The definition held by the symbol entered for a definition node, so
    /// that the types built for a definition share its symbol's node. A node
    /// with no symbol is copied.
    pub fn interned_def<D: InternedDef>(&self, node: &D) -> Arc<D> {
        match self.symbol_map.get(&node.id()).and_then(D::from_symbol) {
            Some(def) => def.clone(),
            None => Arc::new(node.clone()),
        }
    }
}
