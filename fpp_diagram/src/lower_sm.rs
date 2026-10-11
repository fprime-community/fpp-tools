//! Lowering from [`fpp_analysis::Analysis`] state machines into the diagram
//! [`crate::ir::StateMachineDiagram`].
//!
//! The structural transition graph (`sma.transition_graph`) is the source: its
//! `arc_map` keys are the state/choice nodes and its arcs are the transitions.
//! This mirrors the FPP source (nested states and choices preserved) rather than
//! the flattened leaf-state view.

use crate::ir::{SmEdge, SmNode, SmNodeKind, StateMachineDiagram, TransitionActionMode};
use crate::layout::SmLayout;
use crate::lower::LowerError;
use fpp_analysis::Analysis;
use fpp_analysis::semantics::state_machine::transition_graph::{Arc as TgArc, Node as TgNode};
use fpp_analysis::semantics::state_machine::{
    State, StateMachine, StateMachineAnalysis, StateMachineSymbol, StateOrChoice,
};
use fpp_ast::{DefState, DoExpr, StateMember, TransitionExpr, TransitionOrDo};
use fpp_core::{Annotated, Span, Spanned};

/// A source-order sort key for a span: `(file, byte offset)`. Ordering diagram
/// elements by this makes the layout follow the order things appear in the FPP
/// source, so reordering states in the source reorders them in the diagram.
/// Mirrors `fpp_analysis`'s `cmp_span`.
type SpanKey = (String, u32);

fn span_key(span: Span) -> SpanKey {
    (format!("{}", span.file()), span.start().pos())
}

/// Lower a state machine (by fully qualified name) into a diagram.
pub fn lower_state_machine(
    a: &Analysis,
    name: &str,
    mode: TransitionActionMode,
) -> Result<StateMachineDiagram, LowerError> {
    let sm = find_state_machine(a, name).ok_or_else(|| LowerError::NotFound {
        kind: crate::ir::DiagramKind::StateMachine,
        name: name.to_string(),
    })?;
    let sma = &sm.sma;

    let mut nodes = state_and_choice_nodes(sma);
    let mut edges = Vec::new();

    // The synthetic initial pseudo-state and the state-machine-level initial
    // transition into its first state/choice.
    if let Some(initial_target) = &sma.transition_graph.initial_node {
        nodes.push(SmNode {
            id: SmNode::INITIAL_ID.to_string(),
            label: String::new(),
            kind: SmNodeKind::Initial,
            parent: None,
            depth: 0,
            entry_actions: vec![],
            exit_actions: vec![],
            internal_transitions: vec![],
            detail: String::new(),
        });
        edges.push(SmEdge {
            id: "initial".to_string(),
            from: SmNode::INITIAL_ID.to_string(),
            to: node_id(sma, initial_target),
            label: String::new(),
            actions: vec![],
            detail: String::new(),
            choice_branch: None,
        });
    }

    // All transitions. `arc_map` iterates in the order the transition graph was
    // built, which is not source order, so build every edge keyed by its source
    // location, sort by that, then assign sequential ids. Ordering by span makes
    // transitions lay out in source order.
    let mut arc_edges: Vec<(SpanKey, SmEdge)> = sma
        .transition_graph
        .arc_map
        .values()
        .flatten()
        .map(|arc| (span_key(arc_span(arc)), arc_edge(sma, arc, mode)))
        .collect();
    // Tie-break by endpoints so distinct transitions at the same span (e.g. a
    // choice's two branches) still have a stable relative order.
    arc_edges.sort_by(|(ka, a), (kb, b)| {
        ka.cmp(kb)
            .then_with(|| (&a.from, &a.to).cmp(&(&b.from, &b.to)))
    });
    for (seq, (_, mut edge)) in arc_edges.into_iter().enumerate() {
        edge.id = format!("transition.{seq}");
        edges.push(edge);
    }

    // Layout options are read from the state machine definition's pre-annotation
    // (`@ diagram-layout ...`), so the configuration lives in the FPP source.
    let layout = SmLayout::from_annotations(&sm.node.pre_annotation());

    Ok(StateMachineDiagram {
        name: a.get_qualified_name(&sm.get_symbol()),
        nodes,
        edges,
        layout,
    })
}

/// Build a node for every state and choice in the transition graph.
///
/// `arc_map` iterates in the order the transition graph was built, which is not
/// source order, so we sort the resulting nodes by their definition's source
/// location: states lay out in the order they appear in the FPP source, so
/// reordering the source reorders the diagram.
fn state_and_choice_nodes(sma: &StateMachineAnalysis) -> Vec<SmNode> {
    let mut nodes: Vec<(SpanKey, SmNode)> = sma
        .transition_graph
        .arc_map
        .keys()
        .map(|node| {
            let key = span_key(node.soc.get_symbol().get_span());
            (key, soc_node(sma, &node.soc))
        })
        .collect();
    nodes.sort_by(|(ka, _), (kb, _)| ka.cmp(kb));
    nodes.into_iter().map(|(_, node)| node).collect()
}

/// Build an [`SmNode`] for a state-or-choice.
fn soc_node(sma: &StateMachineAnalysis, soc: &StateOrChoice) -> SmNode {
    let symbol = soc.get_symbol();
    let id = sma.get_qualified_name(symbol);
    let label = symbol.get_unqualified_name().to_string();
    let parent = sma
        .parent_state_map
        .get(symbol)
        .map(|p| sma.get_qualified_name(p));
    let depth = node_depth(sma, symbol);

    match soc {
        StateOrChoice::State(StateMachineSymbol::State(def)) => {
            let entry_actions: Vec<String> = State::get_entry_actions(def)
                .iter()
                .map(|i| i.data.clone())
                .collect();
            let exit_actions: Vec<String> = State::get_exit_actions(def)
                .iter()
                .map(|i| i.data.clone())
                .collect();
            let internal_transitions = internal_transitions_of(def);
            let detail = state_detail(&label, &entry_actions, &exit_actions, &internal_transitions);
            SmNode {
                id,
                label,
                kind: SmNodeKind::State,
                parent,
                depth,
                entry_actions,
                exit_actions,
                internal_transitions,
                detail,
            }
        }
        StateOrChoice::Choice(_) => SmNode {
            id,
            label,
            kind: SmNodeKind::Choice,
            parent,
            depth,
            entry_actions: vec![],
            exit_actions: vec![],
            internal_transitions: vec![],
            detail: String::new(),
        },
        // A `State` variant should always wrap a `State` symbol; fall back
        // defensively to a plain state node.
        StateOrChoice::State(_) => SmNode {
            id,
            label,
            kind: SmNodeKind::State,
            parent,
            depth,
            entry_actions: vec![],
            exit_actions: vec![],
            internal_transitions: vec![],
            detail: String::new(),
        },
    }
}

/// The nesting depth of a state/choice: number of ancestor states above it.
fn node_depth(sma: &StateMachineAnalysis, symbol: &StateMachineSymbol) -> u32 {
    let mut depth = 0;
    let mut current = symbol.clone();
    while let Some(parent) = sma.parent_state_map.get(&current) {
        depth += 1;
        current = parent.clone();
    }
    depth
}

/// Build the hover-detail text for a state: its name plus any entry/exit action
/// lists and internal transitions. Empty when the state has nothing extra to
/// reveal.
fn state_detail(name: &str, entry: &[String], exit: &[String], internal: &[String]) -> String {
    if entry.is_empty() && exit.is_empty() && internal.is_empty() {
        return String::new();
    }
    let mut lines = vec![name.to_string()];
    if !entry.is_empty() {
        lines.push(format!("entry / {}", entry.join(" ")));
    }
    if !exit.is_empty() {
        lines.push(format!("exit / {}", exit.join(" ")));
    }
    // Internal transitions (`on sig [guard] do { ... }`) react to a signal
    // without leaving the state; each is already formatted `sig [guard] / a…`.
    lines.extend(internal.iter().cloned());
    lines.join("\n")
}

/// Collect a state's internal transitions — `on signal [guard] do { ... }`
/// specifiers, i.e. `SpecStateTransition`s whose body is a `do` block with no
/// target state. Each is pre-formatted as `signal [guard] / a1 a2` (the guard
/// and action clauses omitted when absent). These never enter the transition
/// graph (they have no target arc), so they must be read directly from the
/// state's AST members here.
fn internal_transitions_of(def: &DefState) -> Vec<String> {
    def.members
        .iter()
        .filter_map(|m| match m {
            StateMember::SpecStateTransition(st) => match &st.transition_or_do {
                TransitionOrDo::Do(d) => Some(format_internal_transition(st, d)),
                // A `... enter Target` transition changes state; it is an arc in
                // the transition graph and rendered as an edge, not here.
                TransitionOrDo::Transition(_) => None,
            },
            _ => None,
        })
        .collect()
}

/// Format an internal transition as `signal [guard] / a1 a2`, dropping the
/// `[guard]` and `/ actions` clauses when they are absent.
fn format_internal_transition(st: &fpp_ast::SpecStateTransition, d: &DoExpr) -> String {
    let mut s = st.signal.data.clone();
    if let Some(guard) = &st.guard {
        s.push_str(&format!(" [{}]", guard.data));
    }
    let actions = action_names(d);
    if !actions.is_empty() {
        s.push_str(&format!(" / {}", actions.join(" ")));
    }
    s
}

/// The source span of a transition-graph arc — the location of the `initial`,
/// `on`, or choice-branch specifier that defines it.
fn arc_span(arc: &TgArc) -> Span {
    match arc {
        TgArc::Initial { a_node, .. } => a_node.span(),
        TgArc::State { a_node, .. } => a_node.span(),
        TgArc::Choice { a_node, .. } => a_node.span(),
    }
}

/// Build an [`SmEdge`] from a transition-graph arc. The `id` is assigned a stable
/// sequence number by the caller after sorting.
fn arc_edge(sma: &StateMachineAnalysis, arc: &TgArc, mode: TransitionActionMode) -> SmEdge {
    let from = node_id(sma, &arc.get_start_node());
    let to = node_id(sma, arc.get_end_node());
    let (label, own_actions) = arc_trigger_and_actions(arc);
    let actions = match mode {
        // UML view: the edge carries only the transition's own `do { }`
        // actions. Entry/exit actions live inside the state (see the node
        // detail / the Mermaid state block), not on the edge.
        TransitionActionMode::Uml => own_actions,
        // Flattened view: the full action sequence that runs when the transition
        // is taken.
        TransitionActionMode::Flattened => flattened_action_names(sma, arc),
    };
    let detail = transition_detail(&label, &actions);
    SmEdge {
        id: String::new(),
        from,
        to,
        label,
        actions,
        detail,
        choice_branch: arc_choice_branch(arc),
    }
}

/// The choice branch an arc represents, or `None` if it does not leave a choice.
/// Determined by node identity (the same test `choice_trigger_and_actions` uses),
/// so it never depends on parsing the `[guard]` / `[!guard]` label.
fn arc_choice_branch(arc: &TgArc) -> Option<crate::ir::ChoiceBranch> {
    let TgArc::Choice {
        start_choice,
        a_node,
        ..
    } = arc
    else {
        return None;
    };
    let StateMachineSymbol::Choice(def) = start_choice else {
        return None;
    };
    // Coerce the arc's transition to `&TransitionExpr` (as `choice_trigger_and_actions`
    // does) so `ptr::eq` compares the two by the same identity test.
    let branch: &TransitionExpr = a_node;
    let is_if =
        std::ptr::eq(&def.if_transition, branch) || def.if_transition.node_id == branch.node_id;
    Some(if is_if {
        crate::ir::ChoiceBranch::Then
    } else {
        crate::ir::ChoiceBranch::Else
    })
}

/// The flattened action names that execute when an arc is taken, per the FPP
/// spec's transition dynamic semantics:
///
/// 1. exit actions of the states left (source leaf first, up to the lowest
///    common ancestor),
/// 2. the transition's own `do { }` actions,
/// 3. entry actions of the intermediate states entered (down to just above the
///    target),
/// 4. the entry actions of the target leaf state itself.
///
/// Steps 1–3 come from the analysis's [`ConstructFlattenedTransition`] (which is
/// codegen-oriented and stops "just above the target", so it omits step 4); we
/// append step 4 here so the diagram reflects the complete executed sequence.
/// A self-transition that stays within a state runs none of these (its common
/// ancestor prefix is stripped and it has no target-leaf entry beyond itself).
fn flattened_action_names(sma: &StateMachineAnalysis, arc: &TgArc) -> Vec<String> {
    use fpp_analysis::passes::state_machine::ConstructFlattenedTransition;
    use fpp_analysis::semantics::state_machine::Transition;

    // The transition's own `do { }` actions, resolved to symbols.
    let raw_actions: Vec<StateMachineSymbol> = match arc {
        TgArc::State { a_node, .. } => match &a_node.transition_or_do {
            TransitionOrDo::Transition(t) => action_symbols_of_transition_expr(sma, t),
            // A `do`-only arc has no target, so it is not in the transition
            // graph; defensively handle it as its literal actions.
            TransitionOrDo::Do(d) => action_symbols(sma, d),
        },
        TgArc::Choice { a_node, .. } => action_symbols_of_transition_expr(sma, a_node),
        TgArc::Initial { a_node, .. } => action_symbols_of_transition_expr(sma, &a_node.transition),
    };

    let source = arc.get_start_node().soc;
    let target = arc.get_end_node().soc.clone();
    let flattened =
        ConstructFlattenedTransition::new(sma, source.clone()).transition(Transition::External {
            actions: raw_actions,
            target: target.clone(),
        });

    let mut names: Vec<String> = flattened
        .get_actions()
        .iter()
        .map(|a| a.get_unqualified_name().to_string())
        .collect();

    // Step 4: the target leaf state's own entry actions, unless this is a
    // self-transition that stays in place (source == target), where re-entry
    // does not occur.
    if source != target
        && let StateOrChoice::State(StateMachineSymbol::State(def)) = &target
    {
        names.extend(State::get_entry_actions(def).iter().map(|i| i.data.clone()));
    }

    names
}

/// Resolve a transition expression's optional `do { }` action idents to symbols.
fn action_symbols_of_transition_expr(
    sma: &StateMachineAnalysis,
    t: &TransitionExpr,
) -> Vec<StateMachineSymbol> {
    t.actions
        .as_ref()
        .map(|d| action_symbols(sma, d))
        .unwrap_or_default()
}

/// Resolve a `do { }` expression's action idents to symbols.
fn action_symbols(sma: &StateMachineAnalysis, d: &DoExpr) -> Vec<StateMachineSymbol> {
    d.actions.iter().map(|a| sma.get_action_symbol(a)).collect()
}

/// Assemble the hover-detail text for a transition: `trigger / a1 a2`, or just
/// `/ a1 a2` when there is no trigger. Empty when there is nothing extra beyond
/// the label.
fn transition_detail(trigger: &str, actions: &[String]) -> String {
    if actions.is_empty() {
        return String::new();
    }
    let joined = actions.join(" ");
    if trigger.is_empty() {
        format!("/ {joined}")
    } else {
        format!("{trigger} / {joined}")
    }
}

/// Compute the `(trigger, actions)` for an arc.
///
/// The `trigger` is the primary label line — `signal [guard]`, or a choice's
/// `[guard]` / `[!guard]`. `actions` is the transition's `do { … }` list, which
/// renderers show as a secondary label line.
fn arc_trigger_and_actions(arc: &TgArc) -> (String, Vec<String>) {
    match arc {
        TgArc::State { a_node, .. } => {
            let mut trigger = a_node.signal.data.clone();
            if let Some(guard) = &a_node.guard {
                trigger.push_str(&format!(" [{}]", guard.data));
            }
            let actions = match &a_node.transition_or_do {
                TransitionOrDo::Transition(t) => actions_of_transition_expr(t),
                TransitionOrDo::Do(d) => action_names(d),
            };
            (trigger, actions)
        }
        TgArc::Choice {
            start_choice,
            a_node,
            ..
        } => choice_trigger_and_actions(start_choice, a_node),
        TgArc::Initial { a_node, .. } => {
            // The initial transition has no trigger; only actions (rare).
            (
                String::new(),
                actions_of_transition_expr(&a_node.transition),
            )
        }
    }
}

/// Compute the `(trigger, actions)` for a choice branch. The choice's guard
/// decides between `if_transition` and `else_transition`.
fn choice_trigger_and_actions(
    start_choice: &StateMachineSymbol,
    branch: &TransitionExpr,
) -> (String, Vec<String>) {
    let actions = actions_of_transition_expr(branch);
    let StateMachineSymbol::Choice(def) = start_choice else {
        return (String::new(), actions);
    };
    // Identify which branch this arc is by node identity.
    let is_if =
        std::ptr::eq(&def.if_transition, branch) || def.if_transition.node_id == branch.node_id;
    let guard = &def.guard.data;
    // The `if` branch is taken when the guard holds; the `else` branch is its
    // negation, shown as `[!guard]` (clearer than `[else guard]`).
    let trigger = if is_if {
        format!("[{guard}]")
    } else {
        format!("[!{guard}]")
    };
    (trigger, actions)
}

/// The action names of a transition expression's optional `do { }`.
fn actions_of_transition_expr(t: &TransitionExpr) -> Vec<String> {
    t.actions.as_ref().map(action_names).unwrap_or_default()
}

/// The action names of a `do { }` expression.
fn action_names(d: &DoExpr) -> Vec<String> {
    d.actions.iter().map(|i| i.data.clone()).collect()
}

/// The diagram node id for a transition-graph node (the qualified name of its
/// state/choice symbol).
fn node_id(sma: &StateMachineAnalysis, node: &TgNode) -> String {
    sma.get_qualified_name(node.soc.get_symbol())
}

/// Find a state machine by its fully qualified name.
///
/// State machines with a blocking analysis error have an incomplete transition
/// graph and are treated as not found for diagramming (they are still stored in
/// the map for editor features such as completion).
fn find_state_machine<'a>(a: &'a Analysis, name: &str) -> Option<&'a StateMachine> {
    a.state_machine_map
        .values()
        .find(|sm| !sm.sma.blocking_error && a.get_qualified_name(&sm.get_symbol()) == name)
}
