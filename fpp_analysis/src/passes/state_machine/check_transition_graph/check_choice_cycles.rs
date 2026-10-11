use crate::analyzers::state_machine::SmResult;
use crate::errors::SemanticError;
use crate::semantics::state_machine::transition_graph::{Arc as TgArc, Node as TgNode};
use crate::semantics::state_machine::{StateMachineAnalysis, StateMachineSymbol, StateOrChoice};
use rustc_hash::FxHashSet as HashSet;
use std::ops::ControlFlow;

/// Checks for choice cycles
pub struct CheckChoiceCycles;

#[derive(Clone, Default)]
struct State {
    #[allow(dead_code)]
    visited: HashSet<StateMachineSymbol>,
    path_set: HashSet<StateMachineSymbol>,
    path_list: Vec<TgArc>,
}

impl State {
    fn clear_path(&self) -> State {
        State {
            visited: self.visited.clone(),
            path_set: HashSet::default(),
            path_list: Vec::new(),
        }
    }
}

/// A choice cycle found by the search: the path from the start choice, ending
/// with the arc that returns to a choice already on the path
struct Cycle {
    /// The choice that the path returns to
    choice: StateMachineSymbol,
    /// The arcs of the path, latest first
    path_list: Vec<TgArc>,
}

impl Cycle {
    /// The arcs that form the cycle itself, without the arcs that lead to it
    fn arcs(&self) -> HashSet<TgArc> {
        let mut arcs = HashSet::default();
        for a in &self.path_list {
            arcs.insert(a.clone());
            if a.get_start_node().soc.get_symbol() == &self.choice {
                break;
            }
        }
        arcs
    }

    /// The error that reports the cycle
    fn error(&self) -> SemanticError {
        let loc = self.choice.get_span();
        let mut parts = vec!["encountered a choice cycle:".to_string()];
        for a in self.path_list.iter().rev() {
            parts.push(a.show_transition());
        }
        SemanticError::ChoiceCycle {
            loc,
            msg: parts.join("\n  "),
        }
    }
}

impl CheckChoiceCycles {
    pub fn state_machine_analysis(sma: &mut StateMachineAnalysis) -> SmResult {
        let nodes: Vec<TgNode> = sma.transition_graph.arc_map.keys().cloned().collect();
        let mut s = State::default();
        let mut reported_cycles: Vec<HashSet<TgArc>> = Vec::new();
        for node in nodes {
            if let StateOrChoice::Choice(c) = node.soc {
                match Self::visit(sma, s.clear_path(), c) {
                    Ok(next) => s = next,
                    Err(cycle) => {
                        // The search reaches a cycle again from every other
                        // choice on it, so emit each cycle once and keep scanning
                        // the remaining choices for other cycles. A cycle is
                        // *blocking*: the typed-element pass walks the choice
                        // graph assuming it is acyclic and would otherwise
                        // recurse forever, so mark it so the gate stops there.
                        let arcs = cycle.arcs();
                        if !reported_cycles.contains(&arcs) {
                            cycle.error().emit();
                            reported_cycles.push(arcs);
                        }
                        sma.blocking_error = true;
                    }
                }
            }
        }
        ControlFlow::Continue(())
    }

    fn visit(sma: &StateMachineAnalysis, s: State, c: StateMachineSymbol) -> Result<State, Cycle> {
        if s.path_set.contains(&c) {
            return Err(Cycle {
                choice: c,
                path_list: s.path_list,
            });
        }
        let path_set = s.path_set.clone();
        let mut s1 = s;
        s1.path_set.insert(c.clone());
        let soc = StateOrChoice::Choice(c.clone());
        let node = TgNode { soc };
        let arcs: Vec<TgArc> = sma
            .transition_graph
            .arc_map
            .get(&node)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default();
        let mut s = s1;
        for a in arcs {
            if let StateOrChoice::Choice(c1) = &a.get_end_node().soc {
                let c1 = c1.clone();
                let mut s2 = s;
                s2.path_list.insert(0, a);
                s = Self::visit(sma, s2, c1)?;
            }
        }
        s.visited.insert(c);
        s.path_set = path_set;
        Ok(s)
    }
}
