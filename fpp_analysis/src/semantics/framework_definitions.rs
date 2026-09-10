use crate::semantics::Symbol;
use rustc_hash::FxHashMap as HashMap;

/// The set of F Prime framework definitions discovered during analysis.
#[derive(Debug, Default, Clone)]
pub struct FrameworkDefinitions {
    /// Map from qualified constant name to its symbol.
    pub constant_map: HashMap<String, Symbol>,
    /// Map from qualified type name to its symbol.
    pub type_map: HashMap<String, Symbol>,
}

impl FrameworkDefinitions {
    pub fn add_constant(&mut self, name: String, sym: Symbol) {
        self.constant_map.insert(name, sym);
    }

    pub fn add_type(&mut self, name: String, sym: Symbol) {
        self.type_map.insert(name, sym);
    }
}
