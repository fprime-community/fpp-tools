//! The `State` enum every internal state machine implicitly defines.

use fpp_ast::{
    ComponentMember, DefEnum, DefEnumConstant, DefState, DefStateMachine, IntegerKind,
    ModuleMember, Name, StateMachineMember, StateMember, TransUnit, TypeName, TypeNameKind,
};
use fpp_core::{Annotated, Node, Spanned};

/// Add state enums to state machines.
///
/// This transform runs after include resolution and before semantic analysis. For
/// each internal state machine it prepends a synthesized
/// `enum State { __FPRIME_UNINITIALIZED, ... }` with one constant per leaf state
/// (named by its qualified path) and an integer representation type sized to the
/// number of constants.
pub fn add_state_enums(ast: &mut TransUnit) {
    for member in &mut ast.0 {
        trans_module_member(member);
    }
}

fn trans_module_member(member: &mut ModuleMember) {
    match member {
        ModuleMember::DefModule(def) => {
            for m in &mut def.members {
                trans_module_member(m);
            }
        }
        ModuleMember::DefComponent(def) => {
            for m in &mut def.members {
                trans_component_member(m);
            }
        }
        ModuleMember::DefStateMachine(def) => add_enum_to_state_machine(def),
        _ => {}
    }
}

fn trans_component_member(member: &mut ComponentMember) {
    if let ComponentMember::DefStateMachine(def) = member {
        add_enum_to_state_machine(def);
    }
}

fn add_enum_to_state_machine(sm: &mut DefStateMachine) {
    // External state machines (no members) get no state enum.
    let members = match &sm.members {
        Some(members) => members,
        None => return,
    };

    // The synthesized nodes take a zero-width location at the start of the
    // state machine definition. Using the state machine's full span would make
    // the synthesized `State` enum (and its constants) cover every inner
    // definition, so position-based lookups (hover, go-to-definition, semantic
    // tokens) would resolve inner items to the synthesized enum instead.
    let sm_span = sm.span();
    let span = fpp_core::Span::new(
        sm_span.file(),
        sm_span.start().pos(),
        0,
        sm_span.including_span(),
    );

    // Collect the leaf-state constants, sorted by name, with the uninitialized
    // state first.
    let mut collected = Vec::new();
    let mut prefix = Vec::new();
    for member in members {
        if let StateMachineMember::DefState(state) = member {
            collect_leaf_states(state, &mut prefix, &mut collected);
        }
    }
    // Deduplicate identical (annotation, name) constants. Two leaf states with
    // the same qualified name collapse to a single enum constant, so a
    // duplicate-state error is not masked by a spurious
    // duplicate-enum-constant error.
    let mut leaf_states: Vec<LeafState> = Vec::new();
    for leaf in collected {
        if !leaf_states
            .iter()
            .any(|u| u.name == leaf.name && u.pre == leaf.pre && u.post == leaf.post)
        {
            leaf_states.push(leaf);
        }
    }
    leaf_states.sort_by(|a, b| a.name.cmp(&b.name));

    let mut constants = Vec::with_capacity(leaf_states.len() + 1);
    constants.push(make_enum_constant(
        "__FPRIME_UNINITIALIZED",
        span,
        vec!["The uninitialized state".to_string()],
        Vec::new(),
    ));
    for leaf in leaf_states {
        constants.push(make_enum_constant(&leaf.name, span, leaf.pre, leaf.post));
    }

    // The representation type is the smallest unsigned integer that fits all
    // the constants.
    let int_kind = if constants.len() < 256 {
        IntegerKind::U8
    } else if constants.len() < 65536 {
        IntegerKind::U16
    } else {
        IntegerKind::U32
    };
    let type_name = TypeName {
        kind: TypeNameKind::Integer(int_kind),
        node_id: Node::new(span),
    };

    let def_enum = DefEnum {
        name: Name {
            data: "State".to_string(),
            node_id: Node::new(span),
        },
        type_name: Some(type_name),
        constants,
        default: None,
        is_dictionary_def: false,
        node_id: Node::new(span),
    };

    // Prepend the enum as the first member of the state machine.
    sm.members
        .as_mut()
        .unwrap()
        .insert(0, StateMachineMember::DefEnum(def_enum));
}

struct LeafState {
    name: String,
    pre: Vec<String>,
    post: Vec<String>,
}

/// The states nested directly inside `state`, without reaching for
/// `fpp_analysis`'s `State::get_substates`, which this crate cannot depend on.
fn substates(state: &DefState) -> Vec<&DefState> {
    state
        .members
        .iter()
        .filter_map(|m| match m {
            StateMember::DefState(node) => Some(node),
            _ => None,
        })
        .collect()
}

fn collect_leaf_states(state: &DefState, prefix: &mut Vec<String>, out: &mut Vec<LeafState>) {
    prefix.push(state.name.data.clone());
    let substates = substates(state);
    if substates.is_empty() {
        out.push(LeafState {
            name: prefix.join("_"),
            pre: state.pre_annotation(),
            post: state.post_annotation(),
        });
    } else {
        for substate in substates {
            collect_leaf_states(substate, prefix, out);
        }
    }
    prefix.pop();
}

fn make_enum_constant(
    name: &str,
    span: fpp_core::Span,
    pre: Vec<String>,
    post: Vec<String>,
) -> DefEnumConstant {
    let constant = DefEnumConstant {
        name: Name {
            data: name.to_string(),
            node_id: Node::new(span),
        },
        value: None,
        node_id: Node::new(span),
    };
    Node::annotate(&constant.node_id, pre, post);
    constant
}
