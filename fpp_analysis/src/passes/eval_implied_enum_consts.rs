use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::component_analyzer::ComponentAnalyzer;
use crate::analyzers::module_analyzer::ModuleAnalyzer;
use crate::analyzers::state_machine_analyzer::StateMachineAnalyzer;
use crate::errors::SemanticError;
use crate::semantics::{EnumConstantValue, Type, Value};
use fpp_ast::{DefEnum, Node, Visitor};
use fpp_core::Spanned;
use std::ops::{ControlFlow, Deref};

/// Evaluate implied enum constants
#[derive(Default)]
pub struct EvalImpliedEnumConsts {
    super_: StateMachineAnalyzer<ComponentAnalyzer<ModuleAnalyzer>>,
}

impl EvalImpliedEnumConsts {
    pub fn new() -> EvalImpliedEnumConsts {
        EvalImpliedEnumConsts::default()
    }
}

impl<'ast> Visitor<'ast> for EvalImpliedEnumConsts {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Self::State, node: Node<'ast>) -> ControlFlow<Self::Break> {
        self.super_.visit(self, a, node)
    }

    fn visit_def_enum(&self, a: &mut Self::State, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        let loc = node.span();
        let constants = &node.constants;
        let num_exprs = constants.iter().filter(|c| c.value.is_some()).count();
        if num_exprs == 0 {
            for (int_value, constant) in constants.iter().enumerate() {
                let enum_type = match a.type_map.get(&constant.node_id).map(|t| t.deref()) {
                    Some(Type::Enum(enum_type)) => enum_type.clone(),
                    // The enum type failed to resolve, and that error is
                    // already reported
                    _ => return ControlFlow::Continue(()),
                };
                let value = EnumConstantValue::new(
                    constant.name.data.clone(),
                    int_value as i128,
                    enum_type,
                );
                a.assign_value(constant.node_id, Value::EnumConstant(value));
            }
        } else if num_exprs != constants.len() {
            SemanticError::InvalidEnumConstants { loc }.emit();
        }
        ControlFlow::Continue(())
    }
}
