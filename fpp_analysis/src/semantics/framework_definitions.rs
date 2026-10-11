use crate::semantics::Symbol;
use indexmap::IndexMap;

/// The F Prime framework definitions present in the model
///
/// The maps keep the definitions in the order they were added, which is the
/// order in which the model defines them.
#[derive(Debug, Default, Clone)]
pub struct FrameworkDefinitions {
    /// Map from qualified constant name to its symbol.
    pub constant_map: IndexMap<String, Symbol>,
    /// Map from qualified type name to its symbol.
    pub type_map: IndexMap<String, Symbol>,
}

impl FrameworkDefinitions {
    pub fn add_constant(&mut self, name: String, sym: Symbol) {
        self.constant_map.insert(name, sym);
    }

    pub fn add_type(&mut self, name: String, sym: Symbol) {
        self.type_map.insert(name, sym);
    }
}
