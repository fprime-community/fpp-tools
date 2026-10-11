use crate::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{Interface, InterfaceInstance, Symbol};
use fpp_ast::AstNode;
use fpp_core::Spanned;
use rustc_hash::FxHashSet;

/// Check that the argument bound to each port interface instance template
/// parameter provides the port interface the parameter declares.
///
/// This runs after the topologies are resolved, because the port interface of a
/// topology used as an argument is not known until then.
pub struct CheckTemplateInterfaceArgs;

impl CheckTemplateInterfaceArgs {
    /// Check all interface parameters template expansions implement the proper interfaces
    pub fn check(&self, a: &Analysis) {
        // The arguments that a resolved topology names as instances. Resolving
        // the topology resolved each of them, and it reported the first one
        // that failed to resolve.
        let topology_args: FxHashSet<&Symbol> = a
            .topology_map
            .values()
            .flat_map(|t| t.direct_template_args.keys())
            .collect();
        for t in a.template_expansion_map.values() {
            let args = t
                .params
                .iter()
                .filter(|param| matches!(param, Symbol::TemplateInterfaceArg { .. }));
            for arg in args {
                if let Err(err) = Self::check_arg(a, arg, topology_args.contains(arg)) {
                    err.emit();
                }
            }
        }
    }

    /// Check one bound port interface instance template parameter.
    ///
    /// `resolved_by_topology` says whether a resolved topology names the
    /// argument as an instance. A failure to resolve such an argument is the
    /// failure that topology's resolution reported, so it is not reported again.
    fn check_arg(a: &Analysis, arg: &Symbol, resolved_by_topology: bool) -> SemanticResult {
        let Symbol::TemplateInterfaceArg { param_def, value } = arg else {
            unreachable!("the arguments are filtered to interface arguments");
        };
        let resolve = || -> SemanticResult<Option<(InterfaceInstance, Interface)>> {
            // Resolve the concrete instance supplied as the argument
            let Some(instance) = a.get_interface_instance(value.id())? else {
                return Ok(None);
            };
            // Resolve the interface declared for this parameter
            let Some(iface) = a.get_interface(param_def.interface.id())? else {
                return Ok(None);
            };
            Ok(Some((instance, iface)))
        };
        let (instance, iface) = match resolve() {
            Ok(Some(resolved)) => resolved,
            // A use that did not resolve was reported by CheckUses
            Ok(None) => return Ok(()),
            Err(_) if resolved_by_topology => return Ok(()),
            Err(err) => return Err(err),
        };
        // An interface instance is built from an entry of the component map or
        // the topology map, so it has a port interface
        let port_interface = instance
            .get_interface(a)
            .expect("an interface instance has a port interface");
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
