use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::Symbol;
use fpp_ast::AstNode;
use fpp_core::Spanned;

/// Check that the argument bound to each port interface instance template
/// parameter provides the port interface the parameter declares.
///
/// This runs after the topologies are resolved, because the port interface of a
/// topology used as an argument is not known until then.
pub struct CheckTemplateInterfaceArgs;

impl CheckTemplateInterfaceArgs {
    /// Check all interface parameters template expansions implement the proper interfaces
    pub fn check(&self, a: &Analysis) {
        for t in a.template_expansion_map.values() {
            let args = t
                .params
                .iter()
                .filter(|param| matches!(param, Symbol::TemplateInterfaceArg { .. }));
            for arg in args {
                if let Err(err) = Self::check_arg(a, arg) {
                    err.emit();
                }
            }
        }
    }

    /// Check one bound port interface instance template parameter
    fn check_arg(a: &Analysis, arg: &Symbol) -> SemanticResult {
        let Symbol::TemplateInterfaceArg { param_def, value } = arg else {
            unreachable!("the arguments are filtered to interface arguments");
        };
        // Resolve the concrete instance supplied as the argument
        let Some(instance) = a.get_interface_instance(value.id())? else {
            return Ok(());
        };
        // Resolve the interface declared for this parameter
        let Some(iface) = a.get_interface(param_def.interface.id())? else {
            return Ok(());
        };
        // An instance whose component failed to check has no port interface
        let Some(port_interface) = instance.get_interface(a) else {
            return Ok(());
        };
        // Make sure the concrete instance provides everything the declared
        // interface requires (the declared interface must be a sub-interface
        // of the concrete instance's port interface)
        port_interface
            .implements(&iface.port_interface)
            .map_err(|err| SemanticError::InterfaceImplements {
                loc: value.span(),
                inner: Box::new(err),
            })
    }
}
