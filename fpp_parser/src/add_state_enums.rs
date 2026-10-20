//! The `State` enum every internal state machine implicitly defines.

use fpp_ast::{
    ComponentMember, DefEnum, DefStateMachine, IntegerKind, ModuleMember, Name, StateMachineMember,
    TransUnit, TypeName, TypeNameKind,
};
use fpp_core::{Diagnostic, Level, Node, Span, Spanned};

/// Add state enums to state machines.
///
/// This transform runs after include resolution and before semantic analysis. For
/// each internal state machine it prepends a synthesized
/// `enum State { __FPRIME_UNINITIALIZED, ... }` with one constant per leaf state
/// (named by its qualified path) and an integer representation type sized to the
/// number of constants. A state machine whose enum cannot be built gets none, and
/// the error is reported.
pub fn add_state_enums(ast: &mut TransUnit) {
    for member in &mut ast.0 {
        module_member(member);
    }
}

fn module_member(member: &mut ModuleMember) {
    match member {
        ModuleMember::DefModule(def) => {
            for m in &mut def.members {
                module_member(m);
            }
        }
        ModuleMember::DefModuleTemplate(def) => {
            for m in &mut def.members {
                module_member(m);
            }
        }
        ModuleMember::SpecTemplateExpand(spec) => {
            for m in spec.members.iter_mut().flatten() {
                module_member(m);
            }
        }
        ModuleMember::DefComponent(def) => {
            for m in &mut def.members {
                component_member(m);
            }
        }
        ModuleMember::DefStateMachine(def) => def_state_machine(def),
        _ => {}
    }
}

fn component_member(member: &mut ComponentMember) {
    if let ComponentMember::DefStateMachine(def) = member {
        def_state_machine(def);
    }
}

/// Add the state enum to an internal state machine. An external state machine
/// (one with no members) gets none.
fn def_state_machine(node: &mut DefStateMachine) {
    if node.members.is_none() {
        return;
    }

    // The synthesized nodes take a zero-width location at the start of the
    // state machine definition. Using the state machine's full span would make
    // the synthesized `State` enum (and its constants) cover every inner
    // definition, so position-based lookups (hover, go-to-definition, semantic
    // tokens) would resolve inner items to the synthesized enum instead.
    let node_span = node.span();
    let loc = Span::new(
        node_span.file(),
        node_span.start().pos(),
        0,
        node_span.including_span(),
    );

    match get_state_enum(node, loc) {
        Ok(state_enum) => node
            .members
            .as_mut()
            .unwrap()
            .insert(0, StateMachineMember::DefEnum(state_enum)),
        Err(err) => Diagnostic::from(err).emit(),
    }
}

/// The representation type of a state enum with `n` constants: the smallest
/// unsigned integer type that holds them
fn get_state_enum_type(n: usize, loc: Span) -> TypeName {
    let type_int = if n < 256 {
        IntegerKind::U8
    } else if n < 65536 {
        IntegerKind::U16
    } else {
        IntegerKind::U32
    };
    TypeName {
        kind: TypeNameKind::Integer(type_int),
        node_id: Node::new(loc),
    }
}

/// The state enum of the state machine `node`, located at `loc`
fn get_state_enum(node: &DefStateMachine, loc: Span) -> Result<DefEnum, TooManyLeafStates> {
    let enum_constants = get_enum_constants::get(node, loc, get_enum_constants::MAX_LEAF_STATES)?;
    let type_name = get_state_enum_type(enum_constants.len(), loc);
    Ok(DefEnum {
        name: Name {
            data: "State".to_string(),
            node_id: Node::new(loc),
        },
        type_name: Some(type_name),
        constants: enum_constants,
        default: None,
        is_dictionary_def: false,
        node_id: Node::new(loc),
    })
}

/// Too many leaf states
struct TooManyLeafStates {
    loc: Span,
}

impl From<TooManyLeafStates> for Diagnostic {
    fn from(value: TooManyLeafStates) -> Diagnostic {
        Diagnostic::new(
            value.loc,
            Level::Error,
            "state machine has too many leaf states",
        )
    }
}

/// The constants of a state enum: one per leaf state
mod get_enum_constants {
    use super::TooManyLeafStates;
    use fpp_ast::{DefEnumConstant, DefState, DefStateMachine, StateMachineMember, StateMember};
    use fpp_core::{Annotated, Node, Span, Spanned};
    use rustc_hash::FxHashSet;

    /// The number of leaf states at which a state enum cannot be built
    pub(super) const MAX_LEAF_STATES: usize = i32::MAX as usize - 1;

    /// An annotated leaf state name
    #[derive(Clone, PartialEq, Eq, Hash)]
    struct AnnotatedName {
        pre: Vec<String>,
        name: String,
        post: Vec<String>,
    }

    struct State {
        /// The current qualifier list in the traversal
        qualifiers: Vec<String>,
        /// The set of annotated state names, in the order they are found
        a_names: Vec<AnnotatedName>,
        /// The members of `a_names`
        a_name_set: FxHashSet<AnnotatedName>,
        /// The size of `a_names` at which a leaf state is an error
        max_leaf_states: usize,
    }

    fn def_state(s: &mut State, state: &DefState) -> Result<(), TooManyLeafStates> {
        s.qualifiers.push(state.name.data.clone());
        let substates = get_substates(state);
        if substates.is_empty() {
            if s.a_names.len() == s.max_leaf_states {
                // This should never happen
                // There should never be over 2 billion leaf states
                return Err(TooManyLeafStates { loc: state.span() });
            }
            let a_name = AnnotatedName {
                pre: state.pre_annotation(),
                name: s.qualifiers.join("_"),
                post: state.post_annotation(),
            };
            if s.a_name_set.insert(a_name.clone()) {
                s.a_names.push(a_name);
            }
        } else {
            for substate in substates {
                def_state(s, substate)?;
            }
        }
        s.qualifiers.pop();
        Ok(())
    }

    /// The states nested directly inside `state`, as `State::get_substates`
    /// finds them in `fpp_analysis`, which this crate cannot depend on
    fn get_substates(state: &DefState) -> Vec<&DefState> {
        state
            .members
            .iter()
            .filter_map(|m| match m {
                StateMember::DefState(node) => Some(node),
                _ => None,
            })
            .collect()
    }

    /// The constants of the state enum of `node`, located at `loc`: the
    /// uninitialized state, then one constant per leaf state, sorted by name.
    /// Identical annotated names are collapsed, as in a set, so two leaf
    /// states with the same qualified name and annotations yield one constant.
    pub(super) fn get(
        node: &DefStateMachine,
        loc: Span,
        max_leaf_states: usize,
    ) -> Result<Vec<DefEnumConstant>, TooManyLeafStates> {
        let mut s = State {
            qualifiers: vec![],
            a_names: vec![],
            a_name_set: FxHashSet::default(),
            max_leaf_states,
        };
        for member in node.members.iter().flatten() {
            if let StateMachineMember::DefState(state) = member {
                def_state(&mut s, state)?;
            }
        }

        let uninit_state = AnnotatedName {
            pre: vec!["The uninitialized state".to_string()],
            name: "__FPRIME_UNINITIALIZED".to_string(),
            post: vec![],
        };
        let mut leaf_states = s.a_names;
        leaf_states.sort_by(|a, b| a.name.cmp(&b.name));
        let constants = std::iter::once(uninit_state).chain(leaf_states);
        Ok(constants
            .map(|a_name| {
                let constant = DefEnumConstant {
                    name: fpp_ast::Name {
                        data: a_name.name,
                        node_id: Node::new(loc),
                    },
                    value: None,
                    node_id: Node::new(loc),
                };
                Node::annotate(&constant.node_id, a_name.pre, a_name.post);
                constant
            })
            .collect())
    }

    #[cfg(test)]
    mod tests {
        use super::get;
        use fpp_ast::{ModuleMember, TransUnit};
        use fpp_core::{CompilerContext, DiagnosticData, DiagnosticEmitter, SourceFile, Spanned};

        struct NoEmitter;

        impl DiagnosticEmitter for NoEmitter {
            fn emit(&mut self, _diagnostic: DiagnosticData) {}
        }

        /// The leaf state count at which the enum cannot be built is checked
        /// before each leaf is added, at the state that would exceed it
        #[test]
        fn too_many_leaf_states() {
            fpp_core::run(&mut CompilerContext::new(NoEmitter), || {
                let source = "state machine S {\n  state A\n  state B {\n    state C\n  }\n}\n";
                let file = SourceFile::new("sm.fpp", source.to_string());
                let tu: TransUnit = crate::parse(file, |p| p.trans_unit(), None);
                let ModuleMember::DefStateMachine(sm) = &tu.0[0] else {
                    panic!("expected a state machine");
                };
                let span = sm.span();

                assert_eq!(get(sm, span, 2).ok().map(|c| c.len()), Some(3));
                let Err(err) = get(sm, span, 1) else {
                    panic!("expected too many leaf states");
                };
                assert_eq!(err.loc.start().line(), 3);
            });
        }
    }
}
