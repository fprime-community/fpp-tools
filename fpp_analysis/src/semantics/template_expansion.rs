use crate::semantics::{NameGroup, Scope, Symbol, SymbolInterface};
use indexmap::IndexMap;
use rustc_hash::{FxBuildHasher, FxHashMap as HashMap};
use std::sync::Arc;

/// An expansion of a module template
#[derive(Debug, Clone)]
pub struct TemplateExpansion {
    /// The AST node defining the template
    pub def_node: Arc<fpp_ast::DefModuleTemplate>,
    /// The AST node expanding the template
    pub expansion: Arc<fpp_ast::SpecTemplateExpand>,
    /// Arguments bound to the parameters of this template during expansion
    pub params: IndexMap<ParamKey, Symbol, FxBuildHasher>,
    /// Scope where template argument symbols are entered
    pub param_scope: Scope,
    /// Scope where symbols defined in the expansion are entered
    pub scope: Scope,
    /// The scopes of the modules defined in the expansion
    pub local_scope_map: HashMap<Symbol, Scope>,
}

/// A key into the parameter map: a name group paired with a parameter name
pub type ParamKey = (NameGroup, String);

impl TemplateExpansion {
    /// Get the name group in which a template argument symbol is entered
    pub fn name_group(symbol: &Symbol) -> NameGroup {
        match symbol {
            Symbol::TemplateConstantArg { .. } => NameGroup::Value,
            Symbol::TemplateTypeArg { .. } => NameGroup::Type,
            Symbol::TemplateInterfaceArg { .. } => NameGroup::PortInterfaceInstance,
            _ => panic!("not a template argument symbol"),
        }
    }

    /// Get the parameter map key for a template argument symbol
    pub fn param_key(symbol: &Symbol) -> ParamKey {
        (Self::name_group(symbol), symbol.name().data.clone())
    }
}
