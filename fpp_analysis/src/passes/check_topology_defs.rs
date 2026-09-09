use crate::Analysis;
use crate::semantics::resolve_topology;
use crate::semantics::{Connection, ConnectionPattern, Symbol, cmp_span};

/// Check topology definitions: resolve direct connections, build the resolved
/// instance map across imports, and check connection membership.
pub struct CheckTopologyDefs;

impl CheckTopologyDefs {
    /// Resolve every topology in dependency order (source order for stability).
    pub fn resolve_all(&self, a: &mut Analysis) {
        let mut symbols: Vec<(Symbol, fpp_core::Span)> = a
            .partial_topology_map
            .iter()
            .map(|(s, t)| (s.clone(), t.loc))
            .collect();
        symbols.sort_by(|x, y| cmp_span(&x.1, &y.1));
        for (sym, _) in symbols {
            self.resolve(a, &sym);
        }
    }

    fn resolve(&self, a: &mut Analysis, symbol: &Symbol) {
        // Topology is already resolved: nothing to do.
        if a.topology_map.contains_key(symbol) {
            return;
        }

        // Resolve directly imported topologies first.
        let deps: Vec<Symbol> = a
            .partial_topology_map
            .get(symbol)
            .map(|t| t.direct_topologies.keys().cloned().collect())
            .unwrap_or_default();
        for dep in &deps {
            self.resolve(a, dep);
        }

        // Visit topology members and compute the unresolved topology.
        let Some(mut top) = a.partial_topology_map.get(symbol).cloned() else {
            return;
        };

        // Resolve raw direct connections (now that imports are resolved).
        let graphs = top.raw_direct_graphs.clone();
        for graph in &graphs {
            let name = graph.name.data.clone();
            for conn in &graph.connections {
                match Connection::from_node(a, conn) {
                    Ok(Some(c)) => top.add_local_connection(&name, c),
                    Ok(None) => {}
                    Err(err) => err.emit(),
                }
            }
        }

        // Resolve raw patterns.
        let patterns = top.raw_patterns.clone();
        for spec in &patterns {
            match ConnectionPattern::from_spec(a, spec) {
                Ok(Some(p)) => {
                    if let Err(err) = top.add_pattern(p) {
                        err.emit();
                    }
                }
                Ok(None) => {}
                Err(err) => err.emit(),
            }
        }

        // Run the full resolution pipeline.
        if let Err(err) = resolve_topology::resolve(a, &mut top) {
            err.emit();
        }

        a.topology_map.insert(symbol.clone(), top);
    }
}
