use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{Interface, PortInstance, SpecialPortInstance};
use fpp_ast::{Ident, SpecialPortInstanceKind};
use fpp_core::{Node, Spanned};
use indexmap::IndexMap;
use rustc_hash::FxBuildHasher;

/// An FPP Port Interface (set of port instances)
#[derive(Debug, Clone)]
pub struct PortInterface {
    /// The type of interface instance this port interface represents
    pub instance_type: String,
    /// The map from port names to port instances, in the order the ports were
    /// added
    pub port_map: IndexMap<String, PortInstance, FxBuildHasher>,
    /// The map from special port kinds to special port instances, in the order
    /// the ports were added
    pub special_port_map: IndexMap<SpecialPortInstanceKind, SpecialPortInstance, FxBuildHasher>,
}

impl PortInterface {
    /// Creates a port interface with no ports
    pub fn new(instance_type: impl Into<String>) -> PortInterface {
        PortInterface {
            instance_type: instance_type.into(),
            port_map: IndexMap::default(),
            special_port_map: IndexMap::default(),
        }
    }

    /// Check if `self` port interface implements `other`
    pub fn implements(&self, other: &PortInterface) -> SemanticResult {
        // Check all the ports in `other` to make sure they exist and match `self`
        for (name, pi) in &other.port_map {
            match self.port_map.get(name) {
                Some(found) => {
                    // Port exists, make sure it matches theirs
                    if !found.signature_eq(pi) {
                        return Err(SemanticError::PortInterfaceInvalidPort {
                            loc: found.get_loc(),
                            def_loc: pi.get_loc(),
                        });
                    }
                }
                None => {
                    return Err(SemanticError::PortInterfaceMissingPort { loc: pi.get_loc() });
                }
            }
        }
        for (kind, pi) in &other.special_port_map {
            match self.special_port_map.get(kind) {
                Some(found) => {
                    // The port exists, make sure it's the same theirs
                    if found.signature() != pi.signature() {
                        return Err(SemanticError::PortInterfaceInvalidPort {
                            loc: found.get_loc(),
                            def_loc: pi.get_loc(),
                        });
                    }
                }
                None => {
                    return Err(SemanticError::PortInterfaceMissingPort { loc: pi.get_loc() });
                }
            }
        }
        Ok(())
    }

    /// Gets a port instance by name, if it is present. Rust-only, for the
    /// lookups that have no name node to report an error at.
    pub fn get_port_instance(&self, name: &str) -> Option<PortInstance> {
        self.port_map.get(name).cloned()
    }

    /// Gets a port instance by name
    pub(crate) fn lookup_port_instance(
        &self,
        name: &Ident,
        interface_name: &str,
    ) -> SemanticResult<PortInstance> {
        self.port_map
            .get(&name.data)
            .cloned()
            .ok_or_else(|| SemanticError::InvalidPortInstanceId {
                loc: name.span(),
                port_name: name.data.clone(),
                instance_type: self.instance_type.clone(),
                interface_name: interface_name.to_string(),
            })
    }

    /// Add a port instance
    pub fn add_port_instance(&self, instance: PortInstance) -> SemanticResult<PortInterface> {
        let mut result = self.update_port_map(instance.clone())?;
        if let PortInstance::Special(special) = instance {
            result = result.update_special_port_map(special)?;
        }
        Ok(result)
    }

    /// Add the ports of an imported interface, marking each as imported through
    /// `import_node`
    pub fn add_imported_interface(
        &self,
        interface: &Interface,
        import_node: Node,
    ) -> SemanticResult<PortInterface> {
        let mut result = self.clone();
        for pi in interface.port_interface.port_map.values() {
            result = match result.add_port_instance(pi.with_import_specifier(import_node)) {
                Ok(c) => c,
                Err(err) => {
                    return Err(SemanticError::InterfaceImport {
                        import_loc: import_node.span(),
                        err: Box::new(err),
                    });
                }
            };
        }
        Ok(result)
    }

    /// Add a port instance to the port map
    fn update_port_map(&self, instance: PortInstance) -> SemanticResult<PortInterface> {
        let name = instance.get_unqualified_name().to_string();
        match self.port_map.get(&name) {
            Some(prev_instance) => Err(SemanticError::DuplicatePortInstance {
                name,
                loc: instance.get_loc(),
                import_locs: instance.get_import_locs(),
                prev_loc: prev_instance.get_loc(),
                prev_import_locs: prev_instance.get_import_locs(),
            }),
            None => {
                let mut result = self.clone();
                result.port_map.insert(name, instance);
                Ok(result)
            }
        }
    }

    /// Add a port instance to the special port map
    fn update_special_port_map(
        &self,
        instance: SpecialPortInstance,
    ) -> SemanticResult<PortInterface> {
        let kind = instance.get_special_kind();
        match self.special_port_map.get(&kind) {
            Some(prev_instance) => Err(SemanticError::DuplicatePortInstance {
                name: kind.to_string(),
                loc: instance.get_loc(),
                import_locs: instance.get_import_locs(),
                prev_loc: prev_instance.get_loc(),
                prev_import_locs: prev_instance.get_import_locs(),
            }),
            None => {
                let mut result = self.clone();
                result.special_port_map.insert(kind, instance);
                Ok(result)
            }
        }
    }
}
