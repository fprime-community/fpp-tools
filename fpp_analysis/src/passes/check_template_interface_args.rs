use crate::Analysis;
use crate::errors::SemanticError;
use crate::semantics::{Symbol, SymbolInterface, cmp_span};
use fpp_ast::AstNode;
use fpp_core::Spanned;

/// Check that the argument bound to each port interface instance template
/// parameter provides the port interface the parameter declares.
///
/// This runs after the topologies are resolved, because the port interface of a
/// topology used as an argument is not known until then.
pub struct CheckTemplateInterfaceArgs;

impl CheckTemplateInterfaceArgs {
    pub fn check(&self, a: &Analysis) {
        // Report in source order: the expansion map is keyed by node.
        let mut args: Vec<Symbol> = a
            .template_expansion_map
            .values()
            .flat_map(|expansion| expansion.params.values().cloned())
            .filter(|param| matches!(param, Symbol::TemplateInterfaceArg { .. }))
            .collect();
        args.sort_by(|x, y| cmp_span(&x.node().span(), &y.node().span()));

        for arg in &args {
            let Symbol::TemplateInterfaceArg { param_def, value } = arg else {
                continue;
            };
            // Resolve the concrete instance supplied as the argument
            let Some(instance) = a.get_interface_instance(value.id()) else {
                continue;
            };
            // Resolve the interface declared for this parameter
            let Some(iface) = a.get_interface(param_def.interface.id()) else {
                continue;
            };
            let Some(port_interface) = instance.get_interface(a) else {
                continue;
            };
            // Make sure the concrete instance provides everything the declared
            // interface requires (the declared interface must be a sub-interface
            // of the concrete instance's port interface)
            if let Err(err) = port_interface.implements(&iface.port_interface) {
                SemanticError::InterfaceImplements {
                    loc: value.span(),
                    inner: Box::new(err),
                }
                .emit();
            }
        }
    }
}
