use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{PortInstance, PortInterface, Symbol, SymbolInterface};
use fpp_ast::{DefInterface, SpecInterfaceImport};
use fpp_core::{Node, Span, Spanned};
use indexmap::IndexMap;
use rustc_hash::FxBuildHasher;
use std::sync::Arc;

/// An FPP interface
#[derive(Debug, Clone)]
pub struct Interface {
    /// The AST node defining the interface
    pub node: Arc<DefInterface>,
    /// The imported interfaces, in the order of their import specifiers:
    /// symbol -> (import node, import location).
    pub import_map: IndexMap<Symbol, (Node, Span), FxBuildHasher>,
    /// The port interface of the interface
    pub port_interface: PortInterface,
}

impl Interface {
    /// Creates an interface with no ports and no imports
    pub fn new(node: Arc<DefInterface>) -> Interface {
        Interface {
            node,
            import_map: IndexMap::default(),
            port_interface: PortInterface::new("interface"),
        }
    }

    /// Gets the unqualified name of the interface
    pub fn get_unqualified_name(&self) -> &str {
        &self.node.name.data
    }

    /// Add a port instance
    pub fn add_port_instance(&self, instance: PortInstance) -> SemanticResult<Interface> {
        let pi = self.port_interface.add_port_instance(instance)?;
        let mut result = self.clone();
        result.port_interface = pi;
        Ok(result)
    }

    /// Add the ports of an imported interface. On failure `self` is left
    /// unchanged, discarding the ports already merged from `interface`.
    pub fn add_imported_interface(
        &mut self,
        interface: &Interface,
        import_node: Node,
    ) -> SemanticResult {
        self.port_interface = self
            .port_interface
            .add_imported_interface(interface, import_node)?;
        Ok(())
    }

    /// Add the symbol of an imported interface
    pub fn add_imported_interface_symbol(
        &self,
        symbol: Symbol,
        import: &SpecInterfaceImport,
    ) -> SemanticResult<Interface> {
        if let Some((_, prev_loc)) = self.import_map.get(&symbol) {
            return Err(SemanticError::DuplicateInterface {
                name: symbol.name().data.clone(),
                loc: import.span(),
                prev_loc: *prev_loc,
            });
        }
        let mut result = self.clone();
        result
            .import_map
            .insert(symbol, (import.node_id, import.span()));
        Ok(result)
    }

    /// The interfaces imported by this one, as (symbol, import specifier node)
    /// pairs in the order of the import specifiers. Rust-only: a copy of the
    /// imports that the resolution can iterate while it updates the interface.
    pub fn imports_in_source_order(&self) -> Vec<(Symbol, Node)> {
        self.import_map
            .iter()
            .map(|(symbol, (node, _))| (symbol.clone(), *node))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::semantics::SymbolInterface;
    use crate::{Analysis, CheckSemantics};
    use fpp_core::{SourceFile, Spanned};

    /// `Interface::imports_in_source_order` lists the imports in the order of
    /// the import specifiers, so `resolve_topology::resolve_interface::resolve`
    /// merges them in that order.
    #[test]
    fn imports_are_merged_in_source_order() {
        const IMPORTS: &str = r#"
interface A {}
interface B {}
interface C {}
interface D {}

interface I {
  import D
  import C
  import B
  import A
}
"#;
        let mut diagnostics = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
        fpp_core::run(&mut ctx, || {
            let source = SourceFile::new("interface_imports.fpp", IMPORTS.to_string());
            let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let mut a = Analysis::new();
            let _ = CheckSemantics::tu_list(
                &mut a,
                fpp_fs::FsReader {},
                std::slice::from_mut(&mut ast),
            );
            let interface = a
                .interface_map
                .values()
                .find(|i| i.get_unqualified_name() == "I")
                .expect("interface I was resolved");
            let imports = interface.imports_in_source_order();
            let names: Vec<&str> = imports
                .iter()
                .map(|(symbol, _)| symbol.name().data.as_str())
                .collect();
            assert_eq!(names, vec!["D", "C", "B", "A"]);
            // The import specifier nodes are in source order too.
            let starts: Vec<_> = imports
                .iter()
                .map(|(_, node)| node.span().start().line())
                .collect();
            assert!(starts.windows(2).all(|w| w[0] < w[1]));
        });
        let output = String::from_utf8(diagnostics).expect("diagnostics are UTF-8");
        assert_eq!(output, "", "expected no diagnostics");
    }
}
