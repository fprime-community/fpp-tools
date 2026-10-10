use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::{InterfaceInstance, Symbol, Topology};
use fpp_ast::AstNode;
use fpp_core::Span;

/// Resolve a topology
pub fn resolve(a: &Analysis, t: &mut Topology) -> SemanticResult {
    {
        let tops: Vec<(Symbol, Span)> = t
            .direct_topologies
            .iter()
            .map(|(s, l)| (s.clone(), *l))
            .collect();
        for (sym, loc) in tops {
            if let Some(dep) = a.topology_map.get(&sym) {
                let ii = InterfaceInstance::from_topology(dep);
                t.add_instance(ii, loc);
            }
        }
    }

    {
        let comps: Vec<(Symbol, Span)> = t
            .direct_component_instances
            .iter()
            .map(|(s, l)| (s.clone(), *l))
            .collect();
        for (sym, loc) in comps {
            if let Some(ci) = a.component_instance_map.get(&sym) {
                let ii = InterfaceInstance::from_component_instance(ci.clone());
                t.add_instance(ii, loc);
            }
        }
    }

    {
        let args: Vec<(Symbol, Span)> = t
            .direct_template_args
            .iter()
            .map(|(s, l)| (s.clone(), *l))
            .collect();
        for (sym, loc) in args {
            let Symbol::TemplateInterfaceArg { param_def, value } = &sym else {
                continue;
            };
            let Some(ii) = a.get_interface_instance(value.id()) else {
                continue;
            };
            let Some(interface) = a.get_interface(param_def.interface.id()) else {
                continue;
            };
            t.add_instance(
                InterfaceInstance::from_template_arg(param_def.clone(), interface, ii),
                loc,
            );
        }
    }

    Ok(())
}
