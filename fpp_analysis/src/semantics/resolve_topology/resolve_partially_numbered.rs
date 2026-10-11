//! Resolve a partially numbered topology

use super::pattern_resolver;
use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::{
    ComponentInstance, Connection, ConnectionPattern, Endpoint, InterfaceInstance, Symbol,
    Topology, TopologyPort,
};
use fpp_core::Span;
use indexmap::{IndexMap, IndexSet};
use std::collections::BTreeMap;

/// Check that connection instances are legal
fn check_connection_instances(t: &Topology) -> SemanticResult {
    let check_connection = |c: &Connection| -> SemanticResult {
        let from_instance = &c.from.port.interface_instance;
        let to_instance = &c.to.port.interface_instance;
        t.look_up_instance_at(from_instance, c.from.loc)?;
        t.look_up_instance_at(to_instance, c.to.loc)?;
        Ok(())
    };
    for c in t.connection_map.values().flatten() {
        check_connection(c)?;
    }
    Ok(())
}

/// Check that connection instances are legal
fn check_port_instances(t: &Topology) -> SemanticResult {
    let check_port = |i: &TopologyPort| -> SemanticResult {
        t.look_up_instance_at(&i.pii.interface_instance, i.get_underlying_port_loc())?;
        Ok(())
    };
    for port in t.port_map.values() {
        check_port(port)?;
    }
    Ok(())
}

/// Check the instances of a pattern
///
/// A pattern names component instances, which may be in the topology directly,
/// through an imported subtopology, or through a bound port interface instance
/// template parameter.
fn check_pattern_instances(t: &Topology, pattern: &ConnectionPattern) -> SemanticResult {
    // Check the source
    {
        let (instance, loc) = &pattern.source;
        t.look_up_component_instance_at(instance, *loc)?;
    }
    // Check the targets
    for (instance, loc) in &pattern.targets {
        t.look_up_component_instance_at(instance, *loc)?;
    }
    Ok(())
}

/// Compute the transitively imported topologies
fn compute_transitive_imports(a: &Analysis, t: &mut Topology) {
    let mut tis: IndexSet<Symbol> = IndexSet::default();
    for ts in t.get_imported_topology_symbols(a) {
        let imported = &a.topology_map[&ts];
        tis.extend(imported.transitive_import_set.iter().cloned());
        tis.insert(ts);
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
    let endpoint_exists = |t: &Topology, endpoint: &Endpoint| -> bool {
        let instance = &endpoint.port.interface_instance;
        // A connection imported from a subtopology names the component
        // instance directly, so an instance that is in this topology only
        // through a bound template parameter still counts
        let component_instance_exists = || match instance.get_component_instance_opt() {
            Some(ci) => t.look_up_component_instance_loc(ci).is_some(),
            None => false,
        };
        t.instance_map.contains_key(instance) || component_instance_exists()
    };
    // Check whether a connection exists
    let exists = |t: &Topology, connection: &Connection| -> bool {
        endpoint_exists(t, &connection.from) && endpoint_exists(t, &connection.to)
    };
    // Import connections from transitively imported topologies
    let symbols: Vec<Symbol> = t.transitive_import_set.iter().cloned().collect();
    for s in symbols {
        for (name, cs) in &a.topology_map[&s].local_connection_map {
            for c in cs {
                if exists(t, c) {
                    t.add_connection(name, c.clone());
                }
            }
        }
    }
}

/// Resolve connections to interface instances of t to component instances
fn resolve_interfaces_to_component_instances(a: &Analysis, t: &mut Topology) {
    fn visit_connection(a: &Analysis, tt: &mut Topology, graph_name: &str, c: &Connection) {
        tt.add_local_connection(
            graph_name,
            Connection {
                from: c.from.get_underlying_endpoint(a),
                to: c.to.get_underlying_endpoint(a),
                is_unmatched: c.is_unmatched,
            },
        );
    }

    fn visit_connection_graph(a: &Analysis, tt: &mut Topology, graph: (&String, &Vec<Connection>)) {
        let (graph_name, connections) = graph;
        for c in connections {
            visit_connection(a, tt, graph_name, c);
        }
    }

    // Clear out connections of T and reprocess them
    // Resolve all port instance identifiers to their 'true' component instance port
    let local_connection_map = std::mem::take(&mut t.local_connection_map);
    t.connection_map = IndexMap::default();
    t.unmatched_connection_set = Vec::new();
    t.output_connection_map = BTreeMap::new();
    t.input_connection_map = BTreeMap::new();
    t.from_port_number_map = BTreeMap::new();
    t.to_port_number_map = BTreeMap::new();
    for graph in &local_connection_map {
        visit_connection_graph(a, t, graph);
    }
}

/// Resolve the instances of t
fn resolve_instances(a: &Analysis, t: &mut Topology) {
    fn import_instance(t: &mut Topology, map_entry: (&InterfaceInstance, &Span)) {
        let (instance, loc) = map_entry;
        t.add_instance(instance.clone(), *loc);
    }
    fn import_instances(a: &Analysis, into: &mut Topology, from_symbol: &Symbol) {
        for map_entry in &a.topology_map[from_symbol].instance_map {
            import_instance(into, map_entry);
        }
    }
    for from_symbol in t.get_imported_topology_symbols(a) {
        import_instances(a, t, &from_symbol);
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
