use crate::analyzers::state_machine::SmResult;
use crate::errors::SemanticError;
use crate::semantics::SymbolInterface;
use crate::semantics::state_machine::StateMachineAnalysis;
use crate::semantics::state_machine::transition_graph::Node as TgNode;
use rustc_hash::FxHashSet as HashSet;
use std::ops::ControlFlow;

/// Check reachability in the transition graph
pub struct CheckTGReachability;

impl CheckTGReachability {
    pub fn state_machine_analysis(sma: &StateMachineAnalysis) -> SmResult {
        let nodes: Vec<TgNode> = sma.transition_graph.arc_map.keys().cloned().collect();
        let reachable_nodes = ReachableNodes::compute(sma);
        for node in nodes {
            if !reachable_nodes.contains(&node) {
                // Unreachable node is non-blocking: emit and keep checking the
                // remaining nodes.
                let loc = node.soc.get_symbol().get_loc();
                let name = node.soc.get_name();
                SemanticError::UnreachableNode { name, loc }.emit();
            }
        }
        ControlFlow::Continue(())
    }
}

struct ReachableNodes;

impl ReachableNodes {
    fn compute(sma: &StateMachineAnalysis) -> HashSet<TgNode> {
        let mut visited = HashSet::default();
        let initial = sma.transition_graph.initial_node.clone().unwrap();
        Self::visit(sma, &mut visited, initial);
        visited
    }

    fn visit(sma: &StateMachineAnalysis, visited: &mut HashSet<TgNode>, node: TgNode) {
        if visited.contains(&node) {
            return;
        }
        visited.insert(node.clone());
        if let Some(arcs) = sma.transition_graph.arc_map.get(&node) {
            let ends: Vec<TgNode> = arcs.iter().map(|arc| arc.get_end_node().clone()).collect();
            for end in ends {
                Self::visit(sma, visited, end);
            }
        }
    }
}
