use crate::Analysis;
use crate::semantics::{Interface, PortInstance, Symbol, resolve_interface};
use fpp_ast::{
    AstNode, DefInterface, DefModule, SpecGeneralPortInstance, SpecInterfaceImport,
    SpecSpecialPortInstance, Visitor, Walkable,
};
use std::ops::ControlFlow;

/// Check interface definitions.
pub struct CheckInterfaceDefs;

impl CheckInterfaceDefs {
    /// Resolve an interface: collect its ports and imports, resolve the
    /// interfaces it directly imports, then merge them in. Results are
    /// memoized in `a.interface_map`.
    fn resolve(&self, a: &mut Analysis, symbol: &Symbol) {
        // Interface is already in the map: nothing to do
        if a.interface_map.contains_key(symbol) {
            return;
        }
        let Symbol::Interface(node) = symbol.clone() else {
            return;
        };

        // Interface is not in the map: visit it
        let saved = a.interface.take();
        a.interface = Some(Interface::new(node.clone()));
        let _ = node.walk(a, self);
        let mut iface = a
            .interface
            .take()
            .expect("interface slot is populated during the walk");
        a.interface = saved;

        // Resolve interfaces directly imported by iface, updating a
        for (import_symbol, _) in iface.imports_in_source_order() {
            self.resolve(a, &import_symbol);
        }

        // Use the updated analysis to resolve iface
        if let Err(err) = resolve_interface(&a.interface_map, &mut iface) {
            err.emit();
        }

        a.interface_map.insert(symbol.clone(), iface);
    }
}

impl<'ast> Visitor<'ast> for CheckInterfaceDefs {
    type Break = ();
    type State = Analysis;

    /// Descend into modules, where interface definitions live.
    fn visit_def_module(
        &self,
        a: &mut Self::State,
        node: &'ast DefModule,
    ) -> ControlFlow<Self::Break> {
        node.walk(a, self)
    }

    fn visit_def_interface(
        &self,
        a: &mut Self::State,
        node: &'ast DefInterface,
    ) -> ControlFlow<Self::Break> {
        let symbol = a.get_symbol(node);
        self.resolve(a, &symbol);
        ControlFlow::Continue(())
    }

    fn visit_spec_general_port_instance(
        &self,
        a: &mut Self::State,
        node: &'ast SpecGeneralPortInstance,
    ) -> ControlFlow<Self::Break> {
        if a.interface.is_some() {
            match PortInstance::from_general(a, node) {
                Ok(instance) => add_resolved_port_instance(a, instance),
                Err(err) => err.emit(),
            }
        }
        ControlFlow::Continue(())
    }

    fn visit_spec_special_port_instance(
        &self,
        a: &mut Self::State,
        node: &'ast SpecSpecialPortInstance,
    ) -> ControlFlow<Self::Break> {
        if a.interface.is_some() {
            match PortInstance::from_special(a, node) {
                Ok(instance) => add_resolved_port_instance(a, instance),
                Err(err) => err.emit(),
            }
        }
        ControlFlow::Continue(())
    }

    fn visit_spec_interface_import(
        &self,
        a: &mut Self::State,
        node: &'ast SpecInterfaceImport,
    ) -> ControlFlow<Self::Break> {
        add_imported_interface(a, node);
        ControlFlow::Continue(())
    }
}

pub(crate) fn add_resolved_port_instance(a: &mut Analysis, instance: PortInstance) {
    if let Some(iface) = a.interface.take() {
        match iface.add_port_instance(instance) {
            Ok(updated) => a.interface = Some(updated),
            Err(err) => {
                a.interface = Some(iface);
                err.emit();
            }
        }
    }
}

fn add_imported_interface(a: &mut Analysis, node: &SpecInterfaceImport) {
    let symbol = match a.use_def_map.get(&node.interface.id()) {
        Some(symbol @ Symbol::Interface(_)) => symbol.clone(),
        _ => return,
    };
    if let Some(iface) = a.interface.take() {
        match iface.add_imported_interface_symbol(symbol, node) {
            Ok(updated) => a.interface = Some(updated),
            Err(err) => {
                a.interface = Some(iface);
                err.emit();
            }
        }
    }
}
