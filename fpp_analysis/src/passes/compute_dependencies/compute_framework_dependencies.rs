use crate::passes::compute_dependencies::FrameworkDependency;
use fpp_ast::{
    ComponentKind, DefComponent, DefInterface, DefModule, InputPortKind, Node,
    SpecGeneralPortInstance, TransUnit, Visitable, Visitor, Walkable,
};
use rustc_hash::FxHashSet as HashSet;
use std::ops::ControlFlow;

/// Compute framework dependencies
pub struct ComputeFrameworkDependencies;

impl ComputeFrameworkDependencies {
    pub fn compute(tul: &[&TransUnit]) -> HashSet<FrameworkDependency> {
        let mut s = HashSet::default();
        for tu in tul {
            let _ = tu.visit(&mut s, &ComputeFrameworkDependencies);
        }
        s
    }
}

impl<'ast> Visitor<'ast> for ComputeFrameworkDependencies {
    type Break = ();
    type State = HashSet<FrameworkDependency>;

    /// Only the containers below descend
    fn super_visit(&self, _s: &mut Self::State, _node: Node<'ast>) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }

    fn visit_def_component(
        &self,
        s: &mut Self::State,
        node: &'ast DefComponent,
    ) -> ControlFlow<Self::Break> {
        match node.kind {
            ComponentKind::Passive => {
                s.insert(FrameworkDependency::FwComp);
            }
            _ => {
                s.insert(FrameworkDependency::FwCompQueued);
                s.insert(FrameworkDependency::Os);
            }
        }
        node.walk(s, self)
    }

    fn visit_def_interface(
        &self,
        s: &mut Self::State,
        node: &'ast DefInterface,
    ) -> ControlFlow<Self::Break> {
        node.walk(s, self)
    }

    fn visit_def_module(
        &self,
        s: &mut Self::State,
        node: &'ast DefModule,
    ) -> ControlFlow<Self::Break> {
        node.walk(s, self)
    }

    fn visit_spec_general_port_instance(
        &self,
        s: &mut Self::State,
        node: &'ast SpecGeneralPortInstance,
    ) -> ControlFlow<Self::Break> {
        if matches!(
            node.kind,
            fpp_ast::GeneralPortInstanceKind::Input(InputPortKind::Guarded)
        ) {
            s.insert(FrameworkDependency::Os);
        }
        ControlFlow::Continue(())
    }

    fn visit_trans_unit(
        &self,
        s: &mut Self::State,
        node: &'ast TransUnit,
    ) -> ControlFlow<Self::Break> {
        node.walk(s, self)
    }
}
