use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::module_analyzer::ModuleAnalyzer;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{QualifiedName, Symbol, SymbolInterface};
use fpp_ast::{
    AstNode, DefAbsType, DefAliasType, DefArray, DefComponent, DefComponentInstance, DefConstant,
    DefEnum, DefInterface, DefModuleTemplate, DefPort, DefStateMachine, DefStruct, DefSystem,
    DefTopology, Node, SpecLocKind, Visitor,
};
use fpp_core::{File, Spanned};
use std::ops::ControlFlow;

/// Check location specifiers
pub struct CheckSpecLocs;

impl<'ast> Visitor<'ast> for CheckSpecLocs {
    type Break = ();
    type State = Analysis;

    fn super_visit(&self, a: &mut Self::State, node: Node<'ast>) -> ControlFlow<Self::Break> {
        ModuleAnalyzer::new().visit(self, a, node)
    }

    fn visit_def_abs_type(
        &self,
        a: &mut Self::State,
        node: &'ast DefAbsType,
    ) -> ControlFlow<Self::Break> {
        Self::check_def(a, SpecLocKind::Type, node)
    }

    fn visit_def_alias_type(
        &self,
        a: &mut Self::State,
        node: &'ast DefAliasType,
    ) -> ControlFlow<Self::Break> {
        Self::check_def(a, SpecLocKind::Type, node)
    }

    fn visit_def_array(
        &self,
        a: &mut Self::State,
        node: &'ast DefArray,
    ) -> ControlFlow<Self::Break> {
        Self::check_def(a, SpecLocKind::Type, node)
    }

    fn visit_def_component(
        &self,
        a: &mut Self::State,
        node: &'ast DefComponent,
    ) -> ControlFlow<Self::Break> {
        Self::check_def(a, SpecLocKind::Component, node)
    }

    fn visit_def_component_instance(
        &self,
        a: &mut Self::State,
        node: &'ast DefComponentInstance,
    ) -> ControlFlow<Self::Break> {
        Self::check_def(a, SpecLocKind::Instance, node)
    }

    fn visit_def_constant(
        &self,
        a: &mut Self::State,
        node: &'ast DefConstant,
    ) -> ControlFlow<Self::Break> {
        Self::check_def(a, SpecLocKind::Constant, node)
    }

    fn visit_def_enum(&self, a: &mut Self::State, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        Self::check_def(a, SpecLocKind::Type, node)
    }

    fn visit_def_interface(
        &self,
        a: &mut Self::State,
        node: &'ast DefInterface,
    ) -> ControlFlow<Self::Break> {
        Self::check_def(a, SpecLocKind::Interface, node)
    }

    fn visit_def_module_template(
        &self,
        a: &mut Self::State,
        node: &'ast DefModuleTemplate,
    ) -> ControlFlow<Self::Break> {
        Self::check_def(a, SpecLocKind::Template, node)
    }

    fn visit_def_port(&self, a: &mut Self::State, node: &'ast DefPort) -> ControlFlow<Self::Break> {
        Self::check_def(a, SpecLocKind::Port, node)
    }

    fn visit_def_state_machine(
        &self,
        a: &mut Self::State,
        node: &'ast DefStateMachine,
    ) -> ControlFlow<Self::Break> {
        Self::check_def(a, SpecLocKind::StateMachine, node)
    }

    fn visit_def_struct(
        &self,
        a: &mut Self::State,
        node: &'ast DefStruct,
    ) -> ControlFlow<Self::Break> {
        Self::check_def(a, SpecLocKind::Type, node)
    }

    fn visit_def_system(
        &self,
        a: &mut Self::State,
        node: &'ast DefSystem,
    ) -> ControlFlow<Self::Break> {
        Self::check_def(a, SpecLocKind::System, node)
    }

    fn visit_def_topology(
        &self,
        a: &mut Self::State,
        node: &'ast DefTopology,
    ) -> ControlFlow<Self::Break> {
        Self::check_def(a, SpecLocKind::Instance, node)
    }
}

impl CheckSpecLocs {
    /// Check the location specifier of a definition, reporting any error
    fn check_def<N: AstNode>(a: &Analysis, kind: SpecLocKind, node: &N) -> ControlFlow<()> {
        if let Some(symbol) = a.get_symbol(node)
            && let Err(err) = Self::check_spec_loc(a, kind, &symbol)
        {
            err.emit();
        }
        ControlFlow::Continue(())
    }

    fn check_spec_loc(a: &Analysis, kind: SpecLocKind, symbol: &Symbol) -> SemanticResult {
        Self::check_path(a, kind.clone(), symbol)?;
        Self::check_dictionary_specifier(a, kind, symbol)
    }

    fn check_path(a: &Analysis, kind: SpecLocKind, symbol: &Symbol) -> SemanticResult {
        let name = symbol.name().data.clone();
        let mut ident_list: Vec<String> = a.scope_name_list.iter().rev().cloned().collect();
        ident_list.push(name);
        let qualified_name = QualifiedName::from(ident_list);
        let actual_loc = symbol.get_loc().tu_location();
        match a.location_specifier_map.get(&(kind, qualified_name)) {
            Some(spec_loc) => {
                let specifier_loc = spec_loc.file.span();
                let specified_java_path = specifier_loc.get_relative_path(&spec_loc.file.data);
                let specified_path = File::Path(specified_java_path).to_string();
                let actual_path = File::from_string(&actual_loc.file().uri()).to_string();
                if specified_path == actual_path {
                    Ok(())
                } else {
                    Err(SemanticError::IncorrectLocationPath {
                        loc: specifier_loc,
                        specified_path,
                        actual_loc,
                    })
                }
            }
            None => Ok(()),
        }
    }

    fn check_dictionary_specifier(
        a: &Analysis,
        kind: SpecLocKind,
        symbol: &Symbol,
    ) -> SemanticResult {
        let name = Analysis::get_qualified_name_from_map(&a.parent_symbol_map, symbol);
        let def_loc = symbol.get_loc();
        match a.location_specifier_map.get(&(kind, name)) {
            Some(spec_loc) => {
                let specifier_loc = spec_loc.span();
                if symbol.is_dictionary_def() == spec_loc.is_dictionary_def {
                    Ok(())
                } else {
                    Err(SemanticError::IncorrectDictionarySpecifier {
                        loc: specifier_loc,
                        def_loc,
                    })
                }
            }
            None => Ok(()),
        }
    }
}
