//! Resolve an interface

use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::{Interface, Symbol};
use fpp_core::Node;

/// Resolve an interface
pub fn resolve(a: &Analysis, i: &mut Interface) -> SemanticResult {
    let resolve_import = |i: &mut Interface, ii: (Symbol, Node)| -> SemanticResult {
        let (symbol, node) = ii;
        // CheckInterfaceDefs enters each imported interface into the map
        // before it resolves the interface that imports it
        let imported = a
            .interface_map
            .get(&symbol)
            .expect("an imported interface is in the interface map");
        i.add_imported_interface(imported, node)
    };
    for ii in i.imports_in_source_order() {
        resolve_import(i, ii)?;
    }
    Ok(())
}
