use crate::Analysis;
use crate::errors::SemanticError;
use crate::passes::check_spec_locs::resolve_spec_path;
use crate::semantics::{QualifiedName, SpecLocEntry};
use fpp_ast::{DefModule, SpecLoc, Visitor, Walkable};
use fpp_core::Spanned;
use std::ops::ControlFlow;

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
        a.scope_name_list.push(node.name.data.clone());
        let result = node.walk(a, self);
        a.scope_name_list.pop();
        result
    }

    fn visit_spec_loc(&self, a: &mut Self::State, node: &'ast SpecLoc) -> ControlFlow<Self::Break> {
        let mut parts: Vec<String> = a.scope_name_list.clone();
        parts.extend(QualifiedName::from(&node.symbol).to_ident_list());
        let key = (node.kind.clone(), parts.join("."));

        let entry = SpecLocEntry {
            spec_span: node.span(),
            file_span: node.file.span(),
            file_value: node.file.data.clone(),
            is_dictionary_def: node.is_dictionary_def,
        };

        match a.location_specifier_map.get(&key) {
            None => {
                a.location_specifier_map.insert(key, entry);
            }
            Some(prev) => {
                let path = resolve_spec_path(entry.file_span, &entry.file_value);
                let prev_path = resolve_spec_path(prev.file_span, &prev.file_value);
                if path != prev_path {
                    SemanticError::InconsistentLocationPath {
                        loc: entry.file_span,
                        path,
                        prev_loc: prev.file_span,
                        prev_path,
                    }
                    .emit();
                } else if entry.is_dictionary_def != prev.is_dictionary_def {
                    SemanticError::InconsistentDictionarySpecifier {
                        loc: entry.spec_span,
                        prev_loc: prev.spec_span,
                    }
                    .emit();
                }
            }
        }

        ControlFlow::Continue(())
    }
}
