use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::nested_analyzer::{NestedAnalyzer, NestedAnalyzerMode};
use crate::errors::SemanticError;
use crate::semantics::{EnumConstantValue, Value};
use fpp_ast::{DefEnum, Node, Visitor};
use fpp_core::Spanned;
use std::ops::ControlFlow;

/// Evaluate implied enum constants
pub struct EvalImpliedEnumConsts<'ast> {
    super_: NestedAnalyzer<'ast, Analysis, Self>,
}

impl<'ast> Default for EvalImpliedEnumConsts<'ast> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> EvalImpliedEnumConsts<'ast> {
    pub fn new() -> EvalImpliedEnumConsts<'ast> {
        EvalImpliedEnumConsts {
            super_: NestedAnalyzer::new(NestedAnalyzerMode::SHALLOW),
        }
    }
}

impl<'ast> Visitor<'ast> for EvalImpliedEnumConsts<'ast> {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Self::State, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    fn visit_def_enum(&self, a: &mut Self::State, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        let enum_type = match a.type_map.get(&node.node_id) {
            Some(ty) => ty.clone(),
            None => return ControlFlow::Continue(()),
        };

        #[allow(clippy::manual_try_fold)]
        node.constants.iter().fold(Some(0), |next, member| {
            match (next, &member.value) {
                (Some(next), Some(_)) => {
                    if next == 0 {
                        // This is the first enum field
                        // All constants should now be explicitly defined
                        None
                    } else {
                        SemanticError::EnumConstantShouldBeImplied { loc: member.span() }.emit();
                        None
                    }
                }
                // Explicitly defined value while not expecting implicitly defined value
                // This is good
                (None, Some(_)) => None,

                // Implicitly defined value while expecting implicitly defined value
                // This is good
                (Some(next), None) => {
                    a.value_map.insert(
                        member.node_id,
                        Value::EnumConstant(EnumConstantValue::new(
                            member.name.data.clone(),
                            next as i128,
                            enum_type.clone(),
                        )),
                    );

                    Some(next + 1)
                }

                // Implicitly defined value while expecting explicitly defined value
                (None, None) => {
                    SemanticError::EnumConstantShouldBeExplicit { loc: member.span() }.emit();
                    None
                }
            }
        });

        ControlFlow::Continue(())
    }
}
