use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::{InterfaceInstance, Symbol, Topology};
use fpp_ast::AstNode;
use fpp_core::Span;

/// Resolve a topology
pub fn resolve(a: &Analysis, t: &mut Topology) -> SemanticResult {
    let tops: Vec<(Symbol, Span)> = t
        .direct_topologies
        .iter()
        .map(|(s, l)| (s.clone(), *l))
        .collect();
    for (ti, loc) in tops {
        let ii = InterfaceInstance::from_topology(&a.topology_map[&ti]);
        t.add_instance(ii, loc);
    }

    let comps: Vec<(Symbol, Span)> = t
        .direct_component_instances
        .iter()
        .map(|(s, l)| (s.clone(), *l))
        .collect();
    for (ci, loc) in comps {
        // An instance whose definition failed to check is not in the map
        if let Some(ci) = a.component_instance_map.get(&ci) {
            let ii = InterfaceInstance::from_component_instance(ci.clone());
            t.add_instance(ii, loc);
        }
    }

    let instances: Vec<(Symbol, Span)> = t
        .direct_template_args
        .iter()
        .map(|(s, l)| (s.clone(), *l))
        .collect();
    for (tip, loc) in instances {
        let Symbol::TemplateInterfaceArg { param_def, value } = &tip else {
            unreachable!("a direct template argument is a template interface argument");
        };
        let Some(ii) = a.get_interface_instance(value.id())? else {
            continue;
        };
        let Some(iface) = a.get_interface(param_def.interface.id())? else {
            continue;
        };
        t.add_instance(
            InterfaceInstance::from_template_arg(param_def.clone(), iface, ii),
            loc,
        );
    }

    Ok(())
}
