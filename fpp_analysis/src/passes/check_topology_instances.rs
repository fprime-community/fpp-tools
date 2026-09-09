use crate::Analysis;
use crate::errors::SemanticError;
use crate::semantics::{PendingTopPort, Symbol, Topology};
use fpp_ast::{
    AstNode, DefModule, DefTopology, SpecDirectConnectionGraph, SpecInstance,
    SpecPatternConnectionGraph, SpecTlmPacketSet, SpecTopPort, Visitor, Walkable,
};
use fpp_core::Spanned;
use std::ops::ControlFlow;

/// Check topology instances: build the partial topology map with the component
/// instances and imported topologies declared in each topology.
pub struct CheckTopologyInstances;

impl<'ast> Visitor<'ast> for CheckTopologyInstances {
    type Break = ();
    type State = Analysis;

    fn visit_def_module(
        &self,
        a: &mut Self::State,
        node: &'ast DefModule,
    ) -> ControlFlow<Self::Break> {
        node.walk(a, self)
    }

    fn visit_def_topology(
        &self,
        a: &mut Self::State,
        node: &'ast DefTopology,
    ) -> ControlFlow<Self::Break> {
        let symbol = a.get_symbol(node);
        // Topology is already in the map: nothing to do.
        if a.partial_topology_map.contains_key(&symbol) {
            return ControlFlow::Continue(());
        }
        let name = a.get_qualified_name(&symbol);
        let prev = a.topology.take();
        a.topology = Some(Topology::new(
            symbol.clone(),
            name,
            node.span(),
            node.implements.clone(),
        ));
        // Visit topology members and compute the unresolved topology.
        let _ = node.walk(a, self);
        if let Some(top) = a.topology.take() {
            a.partial_topology_map.insert(symbol, top);
        }
        a.topology = prev;
        ControlFlow::Continue(())
    }

    fn visit_spec_instance(
        &self,
        a: &mut Self::State,
        node: &'ast SpecInstance,
    ) -> ControlFlow<Self::Break> {
        // An instance/import reference resolves to a component instance symbol
        // (for `instance c`) or a topology symbol (for `import A`). Undefined
        // references are already reported by CheckUses.
        if let Some(symbol) = a.use_def_map.get(&node.instance.id()).cloned()
            && let Some(mut top) = a.topology.take()
        {
            if let Err(err) = top.add_instance_symbol(symbol, node.span()) {
                err.emit();
            }
            a.topology = Some(top);
        }
        ControlFlow::Continue(())
    }

    fn visit_spec_direct_connection_graph(
        &self,
        a: &mut Self::State,
        node: &'ast SpecDirectConnectionGraph,
    ) -> ControlFlow<Self::Break> {
        if let Some(top) = a.topology.as_mut() {
            top.raw_direct_graphs.push(node.clone());
        }
        ControlFlow::Continue(())
    }

    fn visit_spec_pattern_connection_graph(
        &self,
        a: &mut Self::State,
        node: &'ast SpecPatternConnectionGraph,
    ) -> ControlFlow<Self::Break> {
        if let Some(top) = a.topology.as_mut() {
            top.raw_patterns.push(node.clone());
        }
        ControlFlow::Continue(())
    }

    fn visit_spec_tlm_packet_set(
        &self,
        a: &mut Self::State,
        node: &'ast SpecTlmPacketSet,
    ) -> ControlFlow<Self::Break> {
        // Only a deployment topology may specify a telemetry packet set.
        if let Some(top) = a.topology.as_ref()
            && let Symbol::Topology(def) = &top.symbol
            && !def.is_deployment
        {
            SemanticError::InvalidTlmPacketSet {
                loc: node.span(),
                name: node.name.data.clone(),
                msg: "only a deployment topology may specify a telemetry packet set".to_string(),
            }
            .emit();
        }
        ControlFlow::Continue(())
    }

    fn visit_spec_top_port(
        &self,
        a: &mut Self::State,
        node: &'ast SpecTopPort,
    ) -> ControlFlow<Self::Break> {
        if let Some(top) = a.topology.as_mut() {
            top.add_port_node(PendingTopPort {
                name: node.name.data.clone(),
                node_id: node.node_id,
                loc: node.span(),
                underlying_ast: node.underlying_port.clone(),
            });
        }
        ControlFlow::Continue(())
    }
}
