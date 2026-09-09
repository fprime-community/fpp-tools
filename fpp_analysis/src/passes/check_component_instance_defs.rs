use crate::Analysis;
use crate::errors::SemanticError;
use crate::semantics::{ComponentInstance, InitSpecifier};
use fpp_ast::{DefComponentInstance, DefModule, Visitor, Walkable};
use std::ops::ControlFlow;

/// Check component instance definitions.
pub struct CheckComponentInstanceDefs;

impl CheckComponentInstanceDefs {
    /// Ensure that ID ranges do not overlap.
    pub fn check_id_ranges(a: &Analysis) {
        let mut instances: Vec<&ComponentInstance> = a.component_instance_map.values().collect();
        instances.sort_by_key(|ci| ci.base_id);

        let mut idx = 0;
        while idx + 1 < instances.len() {
            let i1 = instances[idx];
            let i2 = instances[idx + 1];
            if i2.max_id >= i2.base_id && i1.base_id >= i2.base_id {
                // Error: i2 has a nonempty ID range, and
                // base id for i1 lies in the ID range of i2
                SemanticError::OverlappingIdRanges {
                    base_id1: i1.base_id,
                    name1: i1.name.clone(),
                    loc1: i1.loc,
                    base_id2: i2.base_id,
                    max_id2: i2.max_id,
                    name2: i2.name.clone(),
                    loc2: i2.loc,
                }
                .emit();
                return;
            } else if i1.max_id < i2.base_id {
                // OK: ID ranges do not overlap; continue checking
                idx += 1;
            } else {
                // Error: Base id for i2 lies in the ID range of i1
                SemanticError::OverlappingIdRanges {
                    base_id1: i2.base_id,
                    name1: i2.name.clone(),
                    loc1: i2.loc,
                    base_id2: i1.base_id,
                    max_id2: i1.max_id,
                    name2: i1.name.clone(),
                    loc2: i1.loc,
                }
                .emit();
                return;
            }
        }
    }
}

impl<'ast> Visitor<'ast> for CheckComponentInstanceDefs {
    type Break = ();
    type State = Analysis;

    fn visit_def_module(
        &self,
        a: &mut Self::State,
        node: &'ast DefModule,
    ) -> ControlFlow<Self::Break> {
        node.walk(a, self)
    }

    fn visit_def_component_instance(
        &self,
        a: &mut Self::State,
        node: &'ast DefComponentInstance,
    ) -> ControlFlow<Self::Break> {
        let instance = match ComponentInstance::from_def(a, node) {
            Ok(Some(instance)) => instance,
            Ok(None) => return ControlFlow::Continue(()),
            Err(err) => {
                err.emit();
                return ControlFlow::Continue(());
            }
        };

        a.component_instance = Some(instance);
        let mut ok = true;
        for spec in &node.init_specs {
            let Some(ci) = a.component_instance.take() else {
                ok = false;
                break;
            };
            match InitSpecifier::from_node(a, spec).and_then(|is| ci.add_init_specifier(is)) {
                Ok(updated) => a.component_instance = Some(updated),
                Err(err) => {
                    err.emit();
                    ok = false;
                    break;
                }
            }
        }

        if ok && let Some(ci) = a.component_instance.take() {
            let symbol = a.get_symbol(node);
            a.component_instance_map.insert(symbol, ci);
        }
        a.component_instance = None;
        ControlFlow::Continue(())
    }
}
