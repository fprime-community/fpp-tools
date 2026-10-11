use crate::Analysis;
use crate::analyzers::state_machine::{
    SmResult, SmTypedElementAnalyzer, StateMachineAnalysisVisitor,
};
use crate::semantics::Type;
use crate::semantics::state_machine::{
    StateMachineAnalysis, StateMachineSymbol, StateMachineTypedElement,
};
use fpp_ast::{
    AstNode, DefChoice, DefStateMachine, Ident, SpecInitialTransition, SpecStateEntry,
    SpecStateExit, SpecStateTransition, TransitionExpr, TransitionOrDo,
};
use fpp_core::Spanned;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Check action and guard types
pub struct CheckActionAndGuardTypes<'a> {
    pub a: &'a Analysis,
}

fn transition_actions(transition: &TransitionExpr) -> &[Ident] {
    match &transition.actions {
        Some(do_expr) => &do_expr.actions,
        None => &[],
    }
}

impl<'a> CheckActionAndGuardTypes<'a> {
    /// Analyze a state machine definition
    pub fn def_state_machine(
        a: &'a Analysis,
        sma: &mut StateMachineAnalysis,
        node: &DefStateMachine,
    ) -> SmResult {
        StateMachineAnalysisVisitor::def_state_machine(&CheckActionAndGuardTypes { a }, sma, node)
    }

    // Check action types
}

impl SmTypedElementAnalyzer for CheckActionAndGuardTypes<'_> {
    fn initial_transition_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        te: &StateMachineTypedElement,
    ) -> SmResult {
        let node = match te {
            StateMachineTypedElement::InitialTransition(node) => node,
            _ => panic!("expected initial transition"),
        };
        let actions = transition_actions(&node.transition);
        self.check_action_types(sma, te, actions)
    }

    fn choice_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        te: &StateMachineTypedElement,
    ) -> SmResult {
        let node = match te {
            StateMachineTypedElement::Choice(node) => node,
            _ => panic!("expected choice"),
        };
        self.check_guard_type(sma, te, &node.guard)?;
        self.check_action_types(sma, te, transition_actions(&node.if_transition))?;
        self.check_action_types(sma, te, transition_actions(&node.else_transition))?;
        ControlFlow::Continue(())
    }

    fn state_entry_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        te: &StateMachineTypedElement,
    ) -> SmResult {
        let node = match te {
            StateMachineTypedElement::StateEntry(node) => node,
            _ => panic!("expected state entry"),
        };
        self.check_action_types(sma, te, &node.actions.actions)
    }

    fn state_exit_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        te: &StateMachineTypedElement,
    ) -> SmResult {
        let node = match te {
            StateMachineTypedElement::StateExit(node) => node,
            _ => panic!("expected state exit"),
        };
        self.check_action_types(sma, te, &node.actions.actions)
    }

    fn state_transition_typed_element(
        &self,
        sma: &mut StateMachineAnalysis,
        te: &StateMachineTypedElement,
    ) -> SmResult {
        let node = match te {
            StateMachineTypedElement::StateTransition(node) => node,
            _ => panic!("expected state transition"),
        };
        if let Some(guard) = &node.guard {
            self.check_guard_type(sma, te, guard)?;
        }
        match &node.transition_or_do {
            TransitionOrDo::Transition(transition) => {
                self.check_action_types(sma, te, transition_actions(transition))?;
            }
            TransitionOrDo::Do(actions) => {
                self.check_action_types(sma, te, &actions.actions)?;
            }
        }
        ControlFlow::Continue(())
    }
}

impl StateMachineAnalysisVisitor for CheckActionAndGuardTypes<'_> {
    fn def_choice(&self, sma: &mut StateMachineAnalysis, node: &DefChoice) -> SmResult {
        self.def_choice_te(sma, node)
    }

    fn spec_state_entry(&self, sma: &mut StateMachineAnalysis, node: &SpecStateEntry) -> SmResult {
        self.spec_state_entry_te(sma, node)
    }

    fn spec_state_exit(&self, sma: &mut StateMachineAnalysis, node: &SpecStateExit) -> SmResult {
        self.spec_state_exit_te(sma, node)
    }

    fn spec_initial_transition(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &SpecInitialTransition,
    ) -> SmResult {
        self.spec_initial_transition_te(sma, node)
    }

    fn spec_state_transition(
        &self,
        sma: &mut StateMachineAnalysis,
        node: &SpecStateTransition,
    ) -> SmResult {
        self.spec_state_transition_te(sma, node)
    }
}

impl CheckActionAndGuardTypes<'_> {
    fn check_action_types(
        &self,
        sma: &mut StateMachineAnalysis,
        te: &StateMachineTypedElement,
        actions: &[Ident],
    ) -> SmResult {
        let a = self.a;
        let get_type_option = |sym: &StateMachineSymbol| -> Option<Arc<Type>> {
            match sym {
                StateMachineSymbol::Action(node) => node
                    .type_name
                    .as_ref()
                    .map(|tn| a.type_map.get(&tn.node_id).unwrap().clone()),
                _ => panic!("expected action symbol"),
            }
        };
        Self::check_call_site_types(sma, te, actions, "action", get_type_option)
    }

    // Check guard types
    fn check_guard_type(
        &self,
        sma: &mut StateMachineAnalysis,
        te: &StateMachineTypedElement,
        guard: &Ident,
    ) -> SmResult {
        let a = self.a;
        let get_type_option = |sym: &StateMachineSymbol| -> Option<Arc<Type>> {
            match sym {
                StateMachineSymbol::Guard(node) => node
                    .type_name
                    .as_ref()
                    .map(|tn| a.type_map.get(&tn.node_id).unwrap().clone()),
                _ => panic!("expected guard symbol"),
            }
        };
        Self::check_call_site_types(
            sma,
            te,
            std::slice::from_ref(guard),
            "guard",
            get_type_option,
        )
    }

    // Check call site types
    fn check_call_site_types(
        sma: &mut StateMachineAnalysis,
        te: &StateMachineTypedElement,
        call_sites: &[Ident],
        site_kind: &str,
        get_type_option: impl Fn(&StateMachineSymbol) -> Option<Arc<Type>>,
    ) -> SmResult {
        let te_kind = te.show_kind().to_string();
        let te_to = sma.type_option_map.get(te).cloned().unwrap();
        for cs in call_sites {
            let loc = cs.span();
            let sym = sma.use_def_map.get(&cs.id()).unwrap().clone();
            let site_to = get_type_option(&sym);
            // Non-blocking: a mismatch is emitted in place and analysis continues.
            sma.convert_type_options_at_call_site(loc, &te_kind, &te_to, site_kind, &site_to);
        }
        ControlFlow::Continue(())
    }
}
