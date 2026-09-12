use crate::Analysis;
use crate::errors::SemanticError;
use crate::passes::check_spec_locs::resolve_spec_path;
use crate::semantics::QualifiedName;
use fpp_ast::{DefModule, SpecLoc, Visitor, Walkable};
use fpp_core::Spanned;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Build the location specifier map, checking that duplicate specifiers for the
/// same symbol name a consistent path.
pub struct BuildSpecLocMap;

impl<'ast> Visitor<'ast> for BuildSpecLocMap {
    type Break = ();
    type State = Analysis;

    fn visit_def_module(
        &self,
        a: &mut Self::State,
        node: &'ast DefModule,
    ) -> ControlFlow<Self::Break> {
        a.scope_name_list.insert(0, node.name.data.clone());
        let result = node.walk(a, self);
        a.scope_name_list.remove(0);
        result
    }

    fn visit_spec_loc(&self, a: &mut Self::State, node: &'ast SpecLoc) -> ControlFlow<Self::Break> {
        let mut parts: Vec<String> = a.scope_name_list.iter().rev().cloned().collect();
        parts.extend(QualifiedName::from(&node.symbol).to_ident_list());
        let key = (node.kind.clone(), QualifiedName::from(parts));

        match a.location_specifier_map.get(&key) {
            None => {
                a.location_specifier_map.insert(key, Arc::new(node.clone()));
            }
            Some(prev) => {
                let path = resolve_spec_path(node.file.span(), &node.file.data);
                let prev_path = resolve_spec_path(prev.file.span(), &prev.file.data);
                if path != prev_path {
                    SemanticError::InconsistentLocationPath {
                        loc: node.file.span(),
                        path,
                        prev_loc: prev.file.span(),
                        prev_path,
                    }
                    .emit();
                } else if node.is_dictionary_def != prev.is_dictionary_def {
                    SemanticError::InconsistentDictionarySpecifier {
                        loc: node.span(),
                        prev_loc: prev.span(),
                    }
                    .emit();
                }
            }
        }

        ControlFlow::Continue(())
    }
}
