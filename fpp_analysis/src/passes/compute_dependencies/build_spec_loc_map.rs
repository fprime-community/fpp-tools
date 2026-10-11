use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::module_analyzer::ModuleAnalyzer;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::QualifiedName;
use fpp_ast::{DefModuleTemplate, Node, SpecLoc, Visitor, Walkable};
use fpp_core::Spanned;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Build the location specifier map
pub struct BuildSpecLocMap;

impl<'ast> Visitor<'ast> for BuildSpecLocMap {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Self::State, node: Node<'ast>) -> ControlFlow<Self::Break> {
        ModuleAnalyzer::new().visit(self, a, node)
    }

    /// A location specifier in the body of a template is analyzed at the point of
    /// the definition of the template, whether or not the template is expanded,
    /// and again, at the point of the expansion, for each expansion.
    fn visit_def_module_template(
        &self,
        a: &mut Self::State,
        node: &'ast DefModuleTemplate,
    ) -> ControlFlow<Self::Break> {
        node.members.walk(a, self)
    }

    fn visit_spec_loc(&self, a: &mut Self::State, node: &'ast SpecLoc) -> ControlFlow<Self::Break> {
        let spec = node;
        let symbol = &spec.symbol;
        let qualified_name = {
            let mut prefix: Vec<String> = if symbol.is_absolute() {
                Vec::new()
            } else {
                a.scope_name_list.iter().rev().cloned().collect()
            };
            prefix.extend(symbol.to_ident_list());
            QualifiedName::from(prefix)
        };
        let key = (spec.kind.clone(), qualified_name);
        match a.location_specifier_map.get(&key) {
            None => {
                a.location_specifier_map.insert(key, Arc::new(spec.clone()));
            }
            Some(spec1) => {
                let result = check_path_consistency(spec, spec1)
                    .and_then(|()| check_dictionary_specifier_consistency(spec, spec1));
                if let Err(err) = result {
                    err.emit();
                }
            }
        }

        ControlFlow::Continue(())
    }
}

fn check_dictionary_specifier_consistency(spec1: &SpecLoc, spec2: &SpecLoc) -> SemanticResult {
    if spec1.is_dictionary_def == spec2.is_dictionary_def {
        Ok(())
    } else {
        Err(SemanticError::InconsistentDictionarySpecifier {
            loc: spec1.span(),
            prev_loc: spec2.span(),
        })
    }
}

fn check_path_consistency(spec1: &SpecLoc, spec2: &SpecLoc) -> SemanticResult {
    let path1 = get_path_string(spec1);
    let path2 = get_path_string(spec2);
    if path1 == path2 {
        Ok(())
    } else {
        Err(SemanticError::InconsistentLocationPath {
            loc: spec1.file.span(),
            path: path1,
            prev_loc: spec2.file.span(),
            prev_path: path2,
        })
    }
}

fn get_path_string(spec: &SpecLoc) -> String {
    let loc = spec.file.span();
    let path = loc.get_relative_path(&spec.file.data);
    path.to_string_lossy().into_owned()
}
