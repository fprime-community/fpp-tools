//! Integration tests for state machine diagram lowering.

use fpp_analysis::{Analysis, CheckSemantics};
use fpp_core::SourceFile;
use fpp_diagram::TransitionActionMode;
use fpp_diagram::ir::{ChoiceBranch, DiagramKind, SmNode, SmNodeKind};

fn with_analysis<R>(src: &str, f: impl FnOnce(&Analysis) -> R) -> R {
    let mut sink = Vec::new();
    let mut ctx = fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut sink));
    fpp_core::run(&mut ctx, || {
        let source = SourceFile::new("test.fpp", src.to_string());
        let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
        let mut a = Analysis::new();
        let _ =
            CheckSemantics::tu_list(&mut a, fpp_fs::FsReader {}, std::slice::from_mut(&mut ast));
        f(&a)
    })
}

/// A state machine with an initial transition, a nested choice, a guard, a
/// signal, an entry action, and leaf states.
const SM: &str = r#"
state machine M {
    guard g
    signal s
    action onEnterS
    initial enter S
    state S {
        entry do { onEnterS }
        on s enter C
        choice C { if g enter S1 else enter S2 }
    }
    state S1 {
        on s enter S
    }
    state S2
}
"#;

#[test]
fn lowers_states_choices_and_initial() {
    with_analysis(SM, |a| {
        let sm = fpp_diagram::lower_state_machine(a, "M", TransitionActionMode::Uml)
            .expect("state machine M should lower");
        assert_eq!(sm.name, "M");

        // Nodes: initial pseudo-state + states S, S1, S2 + choice C.
        let by_id = |id: &str| sm.nodes.iter().find(|n| n.id == id);
        let initial = sm
            .nodes
            .iter()
            .find(|n| n.kind == SmNodeKind::Initial)
            .expect("initial pseudo-state");
        assert_eq!(initial.id, SmNode::INITIAL_ID);

        let s = by_id("S").expect("state S");
        assert_eq!(s.kind, SmNodeKind::State);
        assert_eq!(s.entry_actions, vec!["onEnterS".to_string()]);

        let c = by_id("S.C").expect("nested choice C is qualified under S");
        assert_eq!(c.kind, SmNodeKind::Choice);
        assert_eq!(c.parent.as_deref(), Some("S"));

        assert!(by_id("S1").is_some());
        assert!(by_id("S2").is_some());

        // Nesting depth: top-level states are depth 0; the choice nested under S
        // is depth 1.
        assert_eq!(s.depth, 0);
        assert_eq!(c.depth, 1);
        assert_eq!(by_id("S1").unwrap().depth, 0);
    });
}

#[test]
fn lowers_transitions_with_labels() {
    with_analysis(SM, |a| {
        let sm = fpp_diagram::lower_state_machine(a, "M", TransitionActionMode::Uml)
            .expect("state machine M should lower");

        // The SM-level initial transition: from the initial pseudo-state to S.
        let initial_edge = sm
            .edges
            .iter()
            .find(|e| e.from == SmNode::INITIAL_ID)
            .expect("initial edge");
        assert_eq!(initial_edge.to, "S");

        // The signal transition `on s enter C` from S to the nested choice.
        let s_to_c = sm
            .edges
            .iter()
            .find(|e| e.from == "S" && e.to == "S.C")
            .expect("S -> C transition");
        assert_eq!(s_to_c.label, "s");

        // The choice's two branches, labelled by the guard sense.
        let if_branch = sm
            .edges
            .iter()
            .find(|e| e.from == "S.C" && e.to == "S1")
            .expect("choice if branch");
        assert_eq!(if_branch.label, "[g]");
        assert_eq!(if_branch.choice_branch, Some(ChoiceBranch::Then));
        let else_branch = sm
            .edges
            .iter()
            .find(|e| e.from == "S.C" && e.to == "S2")
            .expect("choice else branch");
        assert_eq!(else_branch.label, "[!g]");
        assert_eq!(else_branch.choice_branch, Some(ChoiceBranch::Else));

        // Ordinary transitions and the initial edge carry no branch polarity.
        assert!(s_to_c.choice_branch.is_none());
        let initial_edge = sm
            .edges
            .iter()
            .find(|e| e.from == SmNode::INITIAL_ID)
            .expect("initial edge");
        assert!(initial_edge.choice_branch.is_none());
    });
}

/// The Mermaid output tags each choice branch with a `%% fpp-choice-edge`
/// metadata comment (using the sanitized node ids) so the webview can color the
/// then/else branches green/red. Mermaid ignores `%%` comments, so the source
/// stays valid.
#[test]
fn mermaid_tags_choice_branches() {
    with_analysis(SM, |a| {
        let mermaid =
            fpp_diagram::lower_state_machine_to_mermaid(a, "M", TransitionActionMode::Uml).unwrap();
        // `S.C` sanitizes to `S_C`, matching the edge's `LS_S_C LE_S1` SVG classes.
        assert!(
            mermaid.contains("%% fpp-choice-edge S_C S1 then"),
            "then branch not tagged:\n{mermaid}"
        );
        assert!(
            mermaid.contains("%% fpp-choice-edge S_C S2 else"),
            "else branch not tagged:\n{mermaid}"
        );
        // Ordinary transitions must not be tagged.
        assert_eq!(
            mermaid.matches("fpp-choice-edge").count(),
            2,
            "only the two choice branches should be tagged:\n{mermaid}"
        );
    });
}

/// A transition that carries actions keeps the trigger as the primary label and
/// exposes the actions separately (as a list and in the hover detail).
#[test]
fn transition_actions_separate_from_trigger() {
    const SM_ACTIONS: &str = r#"
state machine M {
    signal s
    action a1
    action a2
    initial enter A
    state A { on s do { a1, a2 } enter B }
    state B
}
"#;
    with_analysis(SM_ACTIONS, |a| {
        let sm = fpp_diagram::lower_state_machine(a, "M", TransitionActionMode::Uml)
            .expect("state machine M should lower");
        let edge = sm
            .edges
            .iter()
            .find(|e| e.from == "A" && e.to == "B")
            .expect("A -> B transition");
        assert_eq!(edge.label, "s");
        assert_eq!(edge.actions, vec!["a1".to_string(), "a2".to_string()]);
        assert_eq!(edge.detail, "s / a1 a2");

        // The Mermaid edge label stacks the actions under the signal via `<br/>`.
        let mermaid =
            fpp_diagram::lower_state_machine_to_mermaid(a, "M", TransitionActionMode::Uml).unwrap();
        assert!(mermaid.contains("A --> B : s<br/>/ a1 a2"), "\n{mermaid}");
    });
}

/// State machine exercising entry/exit actions across a state boundary, used to
/// contrast the two transition action display modes.
const SM_ACTIONS_XBOUNDARY: &str = r#"
state machine M {
    signal s
    action resetStore
    action onExitReady
    initial enter IDLE
    state IDLE {
        entry do { resetStore }
        on s enter READY
    }
    state READY {
        exit do { onExitReady }
        on s enter IDLE
    }
}
"#;

/// In UML mode, a transition edge shows only the transition's own `do { }`
/// actions (here: none). Entry/exit actions live inside the state, surfaced in
/// the node detail and — for Mermaid — as internal `Id : entry / …` lines.
#[test]
fn uml_mode_keeps_entry_exit_in_states() {
    with_analysis(SM_ACTIONS_XBOUNDARY, |a| {
        let sm = fpp_diagram::lower_state_machine(a, "M", TransitionActionMode::Uml)
            .expect("state machine M should lower");

        // The READY -> IDLE transition has no `do { }` of its own, so the edge
        // carries no actions even though entering IDLE runs `resetStore`.
        let ready_to_idle = sm
            .edges
            .iter()
            .find(|e| e.from == "READY" && e.to == "IDLE")
            .expect("READY -> IDLE transition");
        assert!(
            ready_to_idle.actions.is_empty(),
            "UML edge shows only own do-actions: {:?}",
            ready_to_idle.actions
        );

        // IDLE's entry action lives on the IDLE node.
        let idle = sm
            .nodes
            .iter()
            .find(|n| n.id == "IDLE")
            .expect("state IDLE");
        assert_eq!(idle.entry_actions, vec!["resetStore".to_string()]);

        // Mermaid renders the entry/exit actions as internal description lines.
        let mermaid =
            fpp_diagram::lower_state_machine_to_mermaid(a, "M", TransitionActionMode::Uml)
                .expect("mermaid");
        assert!(mermaid.contains("IDLE : entry / resetStore"), "\n{mermaid}");
        assert!(
            mermaid.contains("READY : exit / onExitReady"),
            "\n{mermaid}"
        );
    });
}

/// A *composite* state cannot carry a description in Mermaid ("Group nodes can
/// only have label"), and a note attached to a group node is hoisted out to the
/// diagram's top level (losing the nesting). So its entry/exit actions are folded
/// into the composite state's label instead, in UML mode.
#[test]
fn uml_mode_composite_state_folds_actions_into_label() {
    const SM_COMPOSITE: &str = r#"
state machine M {
    signal s
    action loadStart
    initial enter IDLE
    state IDLE { on s enter LOADING }
    state LOADING {
        entry do { loadStart }
        initial enter SUB
        state SUB { on s enter IDLE }
    }
}
"#;
    with_analysis(SM_COMPOSITE, |a| {
        let mermaid =
            fpp_diagram::lower_state_machine_to_mermaid(a, "M", TransitionActionMode::Uml)
                .expect("mermaid");

        // The composite state LOADING carries its entry action in its label,
        // stacked under the name via `<br/>`, so it stays inside the box.
        assert!(
            mermaid.contains("state \"LOADING<br/>entry / loadStart\" as LOADING"),
            "\n{mermaid}"
        );

        // It must NOT use a note (Mermaid hoists group-node notes to the top
        // level) or a `LOADING : …` description line (rejected for a group
        // node). (Edges like `IDLE --> LOADING : s` are fine — they are not
        // descriptions of the LOADING node.)
        assert!(
            !mermaid.contains("note right of LOADING"),
            "composite state must not use a note:\n{mermaid}"
        );
        assert!(
            !mermaid
                .lines()
                .any(|l| l.trim_start().starts_with("LOADING :")),
            "composite state must not have a description line:\n{mermaid}"
        );
    });
}

/// In flattened mode, a transition edge shows the full action sequence that runs
/// when it is taken: exit(source) + own do + entry(target, including the target
/// leaf's own entry). The state nodes no longer repeat entry/exit in Mermaid.
#[test]
fn flattened_mode_shows_full_action_sequence() {
    with_analysis(SM_ACTIONS_XBOUNDARY, |a| {
        let sm = fpp_diagram::lower_state_machine(a, "M", TransitionActionMode::Flattened)
            .expect("state machine M should lower");

        // READY -> IDLE: exit READY (onExitReady) then enter IDLE (resetStore).
        let ready_to_idle = sm
            .edges
            .iter()
            .find(|e| e.from == "READY" && e.to == "IDLE")
            .expect("READY -> IDLE transition");
        assert_eq!(
            ready_to_idle.actions,
            vec!["onExitReady".to_string(), "resetStore".to_string()],
            "flattened: exit(READY) then entry(IDLE)"
        );
        assert_eq!(ready_to_idle.detail, "s / onExitReady resetStore");

        // IDLE -> READY: exit IDLE (none) then enter READY (no entry action) —
        // empty, since neither has the relevant action.
        let idle_to_ready = sm
            .edges
            .iter()
            .find(|e| e.from == "IDLE" && e.to == "READY")
            .expect("IDLE -> READY transition");
        assert!(
            idle_to_ready.actions.is_empty(),
            "{:?}",
            idle_to_ready.actions
        );

        // Mermaid does not repeat entry/exit inside the states in this mode.
        let mermaid =
            fpp_diagram::lower_state_machine_to_mermaid(a, "M", TransitionActionMode::Flattened)
                .expect("mermaid");
        assert!(!mermaid.contains("entry /"), "\n{mermaid}");
        assert!(
            mermaid.contains("READY --> IDLE : s<br/>/ onExitReady resetStore"),
            "\n{mermaid}"
        );
    });
}

/// A self-transition stays within its state, so it runs none of that state's
/// entry/exit actions — the common ancestor prefix is stripped — in either mode.
#[test]
fn self_transition_runs_no_entry_exit() {
    const SM_SELF: &str = r#"
state machine M {
    signal s
    action resetStore
    initial enter IDLE
    state IDLE {
        entry do { resetStore }
        on s enter IDLE
    }
}
"#;
    with_analysis(SM_SELF, |a| {
        let sm = fpp_diagram::lower_state_machine(a, "M", TransitionActionMode::Flattened)
            .expect("state machine M should lower");
        let idle_self = sm
            .edges
            .iter()
            .find(|e| e.from == "IDLE" && e.to == "IDLE")
            .expect("IDLE -> IDLE self transition");
        assert!(
            idle_self.actions.is_empty(),
            "self-transition runs no entry/exit: {:?}",
            idle_self.actions
        );
    });
}

/// An internal transition (`on signal [guard] do { ... }`) reacts to a signal
/// without changing state. It has no target arc, so it never appears as a
/// transition edge; instead it is carried on the state (in `internal_transitions`
/// and the hover `detail`) and rendered inside the state box by Mermaid — in
/// both action modes, since there is no edge to fold it into.
#[test]
fn internal_transitions_render_inside_state() {
    const SM_INTERNAL: &str = r#"
state machine M {
    guard g
    signal s
    signal t
    action a1
    action a2
    initial enter IDLE
    state IDLE {
        on s do { a1 }
        on t if g do { a1, a2 }
        on s enter RUN
    }
    state RUN
}
"#;
    for mode in [TransitionActionMode::Uml, TransitionActionMode::Flattened] {
        with_analysis(SM_INTERNAL, |a| {
            let sm = fpp_diagram::lower_state_machine(a, "M", mode)
                .expect("state machine M should lower");
            let idle = sm
                .nodes
                .iter()
                .find(|n| n.id == "IDLE")
                .expect("state IDLE");

            // Both internal transitions are captured, formatted `sig [guard] / a…`.
            assert_eq!(
                idle.internal_transitions,
                vec!["s / a1".to_string(), "t [g] / a1 a2".to_string()],
                "mode {mode:?}"
            );
            // The hover detail lists them under the state name.
            assert!(idle.detail.contains("s / a1"), "{}", idle.detail);
            assert!(idle.detail.contains("t [g] / a1 a2"), "{}", idle.detail);

            // The `on s enter RUN` external transition is still an edge; the two
            // internal transitions are NOT edges.
            assert!(
                sm.edges.iter().any(|e| e.from == "IDLE" && e.to == "RUN"),
                "external transition remains an edge"
            );
            assert!(
                !sm.edges.iter().any(|e| e.from == "IDLE" && e.to == "IDLE"),
                "internal transitions must not become self-edges"
            );

            // Mermaid renders them as inline description lines on the leaf state,
            // in both action modes.
            let mermaid =
                fpp_diagram::lower_state_machine_to_mermaid(a, "M", mode).expect("mermaid");
            assert!(
                mermaid.contains("IDLE : s / a1"),
                "mode {mode:?}:\n{mermaid}"
            );
            assert!(
                mermaid.contains("IDLE : t [g] / a1 a2"),
                "mode {mode:?}:\n{mermaid}"
            );
        });
    }
}

/// A state's entry/exit actions go into `detail`, not the visible label.
#[test]
fn state_entry_actions_go_to_detail() {
    with_analysis(SM, |a| {
        let sm = fpp_diagram::lower_state_machine(a, "M", TransitionActionMode::Uml)
            .expect("state machine M should lower");
        let s = sm.nodes.iter().find(|n| n.id == "S").expect("state S");
        assert_eq!(s.label, "S");
        assert_eq!(s.detail, "S\nentry / onEnterS");
    });
}

#[test]
fn lowers_to_sprotty_state_machine_graph() {
    with_analysis(SM, |a| {
        let model = fpp_diagram::lower_to_smodel(
            a,
            DiagramKind::StateMachine,
            "M",
            false,
            TransitionActionMode::Uml,
        )
        .expect("state machine should lower to sprotty");

        assert_eq!(model["type"], "graph");
        let children = model["children"].as_array().unwrap();

        // The top-level state S, an initial node, and transition edges.
        let s = children
            .iter()
            .find(|c| c["type"] == "node:state" && c["id"] == "S")
            .expect("top-level state S");
        assert!(children.iter().any(|c| c["type"] == "node:initial"));

        // The nested choice S.C is a child of S, not a top-level element.
        assert!(
            !children.iter().any(|c| c["id"] == "S.C"),
            "nested choice must not be top-level"
        );
        let s_children = s["children"].as_array().unwrap();
        assert!(
            s_children
                .iter()
                .any(|c| c["type"] == "node:choice" && c["id"] == "S.C"),
            "choice S.C nested under S: {s_children:?}"
        );

        // The `S -> S.C` transition's LCA is S, so it must be nested inside S
        // (ELK routes it there, with S-relative coordinates), not at the root.
        assert!(
            s_children
                .iter()
                .any(|c| c["type"] == "edge:transition" && c["targetId"] == "S.C"),
            "S -> S.C edge nested under S: {s_children:?}"
        );

        // Edges are distributed across containers; count recursively.
        fn count_transitions(elem: &serde_json::Value) -> usize {
            let mut n = 0;
            if let Some(kids) = elem["children"].as_array() {
                for k in kids {
                    if k["type"] == "edge:transition" {
                        n += 1;
                    }
                    n += count_transitions(k);
                }
            }
            n
        }
        assert!(
            count_transitions(&model) >= 3,
            "expected several transitions across the hierarchy"
        );
    });
}

#[test]
fn missing_state_machine_is_an_error() {
    with_analysis(SM, |a| {
        let err =
            fpp_diagram::lower_state_machine(a, "Nope", TransitionActionMode::Uml).unwrap_err();
        assert!(matches!(err, fpp_diagram::LowerError::NotFound { .. }));
    });
}

/// The `@ diagram-layout ...` annotation on a state machine drives the ELK
/// options embedded in the Mermaid frontmatter, so configuration lives in the
/// FPP source.
#[test]
fn layout_annotation_drives_mermaid_frontmatter() {
    const SM_WITH_LAYOUT: &str = r#"
@ diagram-layout cycleBreaking=GREEDY nodePlacement=NETWORK_SIMPLEX
state machine M {
    signal s
    initial enter A
    state A { on s enter B }
    state B
}
"#;
    with_analysis(SM_WITH_LAYOUT, |a| {
        let sm =
            fpp_diagram::lower_state_machine(a, "M", TransitionActionMode::Uml).expect("lowers");
        assert_eq!(
            sm.layout.cycle_breaking,
            fpp_diagram::layout::CycleBreaking::Greedy
        );

        let mermaid =
            fpp_diagram::lower_state_machine_to_mermaid(a, "M", TransitionActionMode::Uml)
                .expect("mermaid");
        assert!(
            mermaid.contains("cycleBreakingStrategy: GREEDY"),
            "\n{mermaid}"
        );
        assert!(
            mermaid.contains("nodePlacementStrategy: NETWORK_SIMPLEX"),
            "\n{mermaid}"
        );
        // Unspecified option keeps its default.
        assert!(
            mermaid.contains("considerModelOrder: NODES_AND_EDGES"),
            "\n{mermaid}"
        );
        // Engine defaults to ELK.
        assert!(mermaid.contains("layout: elk"), "\n{mermaid}");
    });
}

/// Selecting the `dagre` engine changes the Mermaid `layout`, drops the ELK-only
/// options (which `dagre` ignores), and emits the node/rank spacing block (which
/// only `dagre` honors).
#[test]
fn dagre_engine_omits_elk_options() {
    const SM_WITH_DAGRE: &str = r#"
@ diagram-layout engine=dagre nodeSpacing=80 rankSpacing=100
state machine M {
    signal s
    initial enter A
    state A { on s enter B }
    state B
}
"#;
    with_analysis(SM_WITH_DAGRE, |a| {
        let sm =
            fpp_diagram::lower_state_machine(a, "M", TransitionActionMode::Uml).expect("lowers");
        assert_eq!(sm.layout.engine, fpp_diagram::layout::LayoutEngine::Dagre);

        let mermaid =
            fpp_diagram::lower_state_machine_to_mermaid(a, "M", TransitionActionMode::Uml)
                .expect("mermaid");
        assert!(mermaid.contains("layout: dagre"), "\n{mermaid}");
        // The ELK option block is omitted for the dagre backend.
        assert!(!mermaid.contains("elk:"), "\n{mermaid}");
        assert!(!mermaid.contains("cycleBreakingStrategy"), "\n{mermaid}");
        // The spacing block is emitted for the dagre backend.
        assert!(mermaid.contains("nodeSpacing: 80"), "\n{mermaid}");
        assert!(mermaid.contains("rankSpacing: 100"), "\n{mermaid}");
    });
}

/// The flow direction applies to both engines and is emitted as a `direction`
/// statement in the diagram body. The spacing block is ELK-suppressed.
#[test]
fn direction_is_emitted_for_both_engines() {
    const SM_LR: &str = r#"
@ diagram-layout direction=LR
state machine M {
    signal s
    initial enter A
    state A { on s enter B }
    state B
}
"#;
    with_analysis(SM_LR, |a| {
        let sm =
            fpp_diagram::lower_state_machine(a, "M", TransitionActionMode::Uml).expect("lowers");
        assert_eq!(
            sm.layout.direction,
            fpp_diagram::layout::Direction::LeftRight
        );

        let mermaid =
            fpp_diagram::lower_state_machine_to_mermaid(a, "M", TransitionActionMode::Uml)
                .expect("mermaid");
        assert!(mermaid.contains("direction LR"), "\n{mermaid}");
        // Default engine is ELK, so the dagre-only spacing block is absent.
        assert!(!mermaid.contains("nodeSpacing:"), "\n{mermaid}");
    });
}

#[test]
fn generates_mermaid_state_diagram() {
    with_analysis(SM, |a| {
        let mermaid =
            fpp_diagram::lower_state_machine_to_mermaid(a, "M", TransitionActionMode::Uml)
                .expect("state machine M should generate mermaid");

        // YAML frontmatter embeds the layout config, then the diagram header.
        assert!(mermaid.starts_with("---\n"), "\n{mermaid}");
        assert!(mermaid.contains("layout: elk"), "\n{mermaid}");
        assert!(
            mermaid.contains("cycleBreakingStrategy: MODEL_ORDER"),
            "\n{mermaid}"
        );
        assert!(mermaid.contains("\nstateDiagram-v2\n"), "\n{mermaid}");

        // The SM-level initial transition into S.
        assert!(mermaid.contains("[*] --> S"), "\n{mermaid}");

        // The nested choice C is a labeled node (styled as a decision) inside
        // S's block, so its name is visible (a bare `<<choice>>` diamond has no
        // label).
        assert!(mermaid.contains("state S {"), "\n{mermaid}");
        assert!(mermaid.contains("state \"C\" as S_C"), "\n{mermaid}");
        assert!(mermaid.contains("class S_C choice"), "\n{mermaid}");

        // A signal transition from S to the choice, labelled with the signal.
        assert!(mermaid.contains("S --> S_C : s"), "\n{mermaid}");

        // The choice's two guarded branches.
        assert!(mermaid.contains("S_C --> S1 : [g]"), "\n{mermaid}");
        assert!(mermaid.contains("S_C --> S2 : [!g]"), "\n{mermaid}");

        // Display label for a state preserves its unqualified name. S is a
        // composite state with an entry action, so its actions are folded into
        // the label (stacked under the name via `<br/>`).
        assert!(
            mermaid.contains("state \"S<br/>entry / onEnterS\" as S"),
            "\n{mermaid}"
        );
    });
}

/// The generated Mermaid must be byte-for-byte identical across runs. The
/// transition graph is stored in hash maps/sets with no stable iteration order,
/// so the lowering sorts nodes and edges; without that the layout would jump
/// around between renders.
#[test]
fn mermaid_output_is_deterministic() {
    // Re-run the whole analysis+lowering several times; a fresh analysis each
    // time gives the hash containers fresh (randomized) iteration orders.
    let outputs: Vec<String> = (0..8)
        .map(|_| {
            with_analysis(SM, |a| {
                fpp_diagram::lower_state_machine_to_mermaid(a, "M", TransitionActionMode::Uml)
                    .expect("lowers")
            })
        })
        .collect();
    for (i, out) in outputs.iter().enumerate() {
        assert_eq!(
            out, &outputs[0],
            "run {i} differs:\n{out}\n---\n{}",
            outputs[0]
        );
    }
}

/// Node declaration order follows the source order of the state definitions, so
/// reordering states in the FPP source reorders them in the diagram.
#[test]
fn node_order_follows_source_order() {
    const A_FIRST: &str = r#"
state machine M {
    signal s
    initial enter A
    state A { on s enter B }
    state B { on s enter A }
}
"#;
    const B_FIRST: &str = r#"
state machine M {
    signal s
    initial enter A
    state B { on s enter A }
    state A { on s enter B }
}
"#;
    // Position of each state's declaration line in the generated Mermaid.
    let decl_order = |src: &str| -> (usize, usize) {
        with_analysis(src, |a| {
            let m = fpp_diagram::lower_state_machine_to_mermaid(a, "M", TransitionActionMode::Uml)
                .unwrap();
            let a_pos = m.find("state \"A\" as A").expect("A declared");
            let b_pos = m.find("state \"B\" as B").expect("B declared");
            (a_pos, b_pos)
        })
    };
    let (a1, b1) = decl_order(A_FIRST);
    assert!(a1 < b1, "A defined first should be declared first");
    let (a2, b2) = decl_order(B_FIRST);
    assert!(b2 < a2, "B defined first should be declared first");
}
