use super::pattern_resolver;
use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::{
    ComponentInstance, Connection, ConnectionPattern, InterfaceInstance, Symbol, Topology,
};
use fpp_core::Span;
use rustc_hash::FxHashSet as HashSet;

// Resolve a partially numbered topology

/// Check that connection instances are legal
fn check_connection_instances(t: &Topology) -> SemanticResult {
    let check_connection = |c: &Connection| -> SemanticResult {
        let from_instance = &c.from.port.interface_instance;
        let to_instance = &c.to.port.interface_instance;
        t.look_up_instance_at(from_instance, c.from.loc)?;
        t.look_up_instance_at(to_instance, c.to.loc)?;
        Ok(())
    };
    for cs in t.connection_map.values() {
        for c in cs {
            check_connection(c)?;
        }
    }
    Ok(())
}

/// Check that connection instances are legal
fn check_port_instances(t: &Topology) -> SemanticResult {
    for i in t.port_map.values() {
        t.look_up_instance_at(&i.pii.interface_instance, i.get_underlying_port_loc())?;
    }
    Ok(())
}

/// Check the instances of a pattern
fn check_pattern_instances(t: &Topology, pattern: &ConnectionPattern) -> SemanticResult {
    // Check the source
    {
        let (instance, loc) = &pattern.source;
        t.look_up_instance_at(
            &InterfaceInstance::from_component_instance(instance.clone()),
            *loc,
        )?;
    }
    // Check the targets
    for (instance, loc) in &pattern.targets {
        t.look_up_instance_at(
            &InterfaceInstance::from_component_instance(instance.clone()),
            *loc,
        )?;
    }
    Ok(())
}

/// Compute the transitively imported topologies
fn compute_transitive_imports(a: &Analysis, t: &mut Topology) {
    let mut tis: HashSet<Symbol> = HashSet::default();
    for ts in t.direct_topologies.keys() {
        if let Some(dep) = a.topology_map.get(ts) {
            for s in &dep.transitive_import_set {
                tis.insert(s.clone());
            }
        }
        tis.insert(ts.clone());
    }
    t.transitive_import_set = tis;
}

/// Resolve the connection patterns of t
fn resolve_patterns(a: &Analysis, t: &mut Topology) -> SemanticResult {
    let patterns: Vec<ConnectionPattern> = t.pattern_map.values().cloned().collect();
    for p in patterns {
        let instances: Vec<ComponentInstance> = t.component_instance_map().into_keys().collect();
        check_pattern_instances(t, &p)?;
        let connections = pattern_resolver::resolve(a, &p, &instances)?;
        for (name, c) in connections {
            // Skip this connection if it already exists
            // For example, it could be imported
            if !t.connection_exists_between(&c.from.port, &c.to.port) {
                t.add_local_connection(&name, c);
            }
        }
    }
    Ok(())
}

/// Resolve the imported connections of t
fn resolve_imported_connections(a: &Analysis, t: &mut Topology) {
    // Check whether an instance exists
    let endpoint_exists = |t: &Topology, endpoint: &crate::semantics::Endpoint| -> bool {
        t.instance_map
            .contains_key(&endpoint.port.interface_instance)
    };
    // Import connections from transitively imported topologies
    let syms: Vec<Symbol> = t.transitive_import_set.iter().cloned().collect();
    for s in syms {
        let Some(dep) = a.topology_map.get(&s) else {
            continue;
        };
        let graphs: Vec<(String, Vec<Connection>)> = dep
            .local_connection_map
            .iter()
            .map(|(name, cs)| (name.clone(), cs.clone()))
            .collect();
        for (name, cs) in graphs {
            for c in cs {
                // Check whether a connection exists
                if endpoint_exists(t, &c.from) && endpoint_exists(t, &c.to) {
                    t.add_connection(&name, c);
                }
            }
        }
    }
}

/// Resolve connections to interface instances of t to component instances
fn resolve_interfaces_to_component_instances(a: &Analysis, t: &mut Topology) {
    // Clear out connections of T and reprocess them
    // Resolve all port instance identifiers to their 'true' component instance port
    let graphs: Vec<(String, Vec<Connection>)> = t
        .local_connection_map
        .iter()
        .map(|(name, cs)| (name.clone(), cs.clone()))
        .collect();
    t.clear_connections();
    for (graph_name, cs) in graphs {
        for c in cs {
            t.add_local_connection(
                &graph_name,
                Connection {
                    from: c.from.get_underlying_endpoint(a),
                    to: c.to.get_underlying_endpoint(a),
                    is_unmatched: c.is_unmatched,
                },
            );
        }
    }
}

/// Resolve the instances of t
fn resolve_instances(a: &Analysis, t: &mut Topology) {
    let syms: Vec<Symbol> = t.direct_topologies.keys().cloned().collect();
    for from_symbol in syms {
        if let Some(dep) = a.topology_map.get(&from_symbol) {
            let entries: Vec<(InterfaceInstance, Span)> = dep
                .instance_map
                .iter()
                .map(|(ii, l)| (ii.clone(), *l))
                .collect();
            for (instance, loc) in entries {
                t.add_instance(instance, loc);
            }
        }
    }
}

/// Resolve this topology to a partially numbered topology
pub fn resolve(a: &Analysis, t: &mut Topology) -> SemanticResult {
    compute_transitive_imports(a, t);
    resolve_instances(a, t);
    check_port_instances(t)?;
    check_connection_instances(t)?;
    resolve_interfaces_to_component_instances(a, t);
    resolve_imported_connections(a, t);
    resolve_patterns(a, t)?;
    Ok(())
}
