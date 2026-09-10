use crate::Analysis;
use crate::errors::SemanticError;
use fpp_ast::{
    AstNode, DefAbsType, DefAliasType, DefArray, DefComponent, DefComponentInstance, DefConstant,
    DefEnum, DefInterface, DefModule, DefPort, DefStateMachine, DefStruct, DefSystem, DefTopology,
    SpecLocKind, Visitor, Walkable,
};
use fpp_core::{Span, Spanned};
use std::ops::ControlFlow;

/// Resolve a location-specifier path relative to the directory of the file that
/// contains the specifier's file string literal.
pub(crate) fn resolve_spec_path(file_span: Span, specified: &str) -> String {
    let uri = file_span.file().uri();
    match std::path::Path::new(&uri).parent() {
        Some(dir) => dir.join(specified).to_string_lossy().into_owned(),
        None => specified.to_string(),
    }
}

/// Follow the include chain up to the enclosing translation unit.
pub(crate) fn tu_span(mut span: Span) -> Span {
    while let Some(including) = span.including_span() {
        span = including;
    }
    span
}

/// Check location specifiers against the actual definition locations.
pub struct CheckSpecLocs;

impl CheckSpecLocs {
    fn check_spec_loc<N: AstNode + Spanned>(&self, a: &Analysis, kind: SpecLocKind, node: &N) {
        let Some(symbol) = a.get_symbol(node) else {
            return;
        };
        let qualified_name = a.get_qualified_name(&symbol);
        let Some(entry) = a.location_specifier_map.get(&(kind, qualified_name)) else {
            return;
        };

        // Check the path
        let specified_path = resolve_spec_path(entry.file_span, &entry.file_value);
        let actual_span = tu_span(node.span());
        let actual_path = actual_span.file().uri();
        if specified_path != actual_path {
            SemanticError::IncorrectLocationPath {
                loc: entry.file_span,
                specified_path,
                actual_loc: actual_span,
            }
            .emit();
            return;
        }

        // Check the dictionary specifier
        if symbol.is_dictionary_def() != entry.is_dictionary_def {
            SemanticError::IncorrectDictionarySpecifier {
                loc: entry.spec_span,
                def_loc: node.span(),
            }
            .emit();
        }
    }
}

impl<'ast> Visitor<'ast> for CheckSpecLocs {
    type Break = ();
    type State = Analysis;

    fn visit_def_module(
        &self,
        a: &mut Self::State,
        node: &'ast DefModule,
    ) -> ControlFlow<Self::Break> {
        node.walk(a, self)
    }

    fn visit_def_abs_type(
        &self,
        a: &mut Self::State,
        node: &'ast DefAbsType,
    ) -> ControlFlow<Self::Break> {
        self.check_spec_loc(a, SpecLocKind::Type, node);
        ControlFlow::Continue(())
    }

    fn visit_def_alias_type(
        &self,
        a: &mut Self::State,
        node: &'ast DefAliasType,
    ) -> ControlFlow<Self::Break> {
        self.check_spec_loc(a, SpecLocKind::Type, node);
        ControlFlow::Continue(())
    }

    fn visit_def_array(
        &self,
        a: &mut Self::State,
        node: &'ast DefArray,
    ) -> ControlFlow<Self::Break> {
        self.check_spec_loc(a, SpecLocKind::Type, node);
        ControlFlow::Continue(())
    }

    fn visit_def_enum(&self, a: &mut Self::State, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        self.check_spec_loc(a, SpecLocKind::Type, node);
        ControlFlow::Continue(())
    }

    fn visit_def_struct(
        &self,
        a: &mut Self::State,
        node: &'ast DefStruct,
    ) -> ControlFlow<Self::Break> {
        self.check_spec_loc(a, SpecLocKind::Type, node);
        ControlFlow::Continue(())
    }

    fn visit_def_component(
        &self,
        a: &mut Self::State,
        node: &'ast DefComponent,
    ) -> ControlFlow<Self::Break> {
        self.check_spec_loc(a, SpecLocKind::Component, node);
        ControlFlow::Continue(())
    }

    fn visit_def_component_instance(
        &self,
        a: &mut Self::State,
        node: &'ast DefComponentInstance,
    ) -> ControlFlow<Self::Break> {
        self.check_spec_loc(a, SpecLocKind::Instance, node);
        ControlFlow::Continue(())
    }

    fn visit_def_constant(
        &self,
        a: &mut Self::State,
        node: &'ast DefConstant,
    ) -> ControlFlow<Self::Break> {
        self.check_spec_loc(a, SpecLocKind::Constant, node);
        ControlFlow::Continue(())
    }

    fn visit_def_interface(
        &self,
        a: &mut Self::State,
        node: &'ast DefInterface,
    ) -> ControlFlow<Self::Break> {
        self.check_spec_loc(a, SpecLocKind::Interface, node);
        ControlFlow::Continue(())
    }

    fn visit_def_port(&self, a: &mut Self::State, node: &'ast DefPort) -> ControlFlow<Self::Break> {
        self.check_spec_loc(a, SpecLocKind::Port, node);
        ControlFlow::Continue(())
    }

    fn visit_def_state_machine(
        &self,
        a: &mut Self::State,
        node: &'ast DefStateMachine,
    ) -> ControlFlow<Self::Break> {
        self.check_spec_loc(a, SpecLocKind::StateMachine, node);
        ControlFlow::Continue(())
    }

    fn visit_def_topology(
        &self,
        a: &mut Self::State,
        node: &'ast DefTopology,
    ) -> ControlFlow<Self::Break> {
        self.check_spec_loc(a, SpecLocKind::Instance, node);
        ControlFlow::Continue(())
    }

    fn visit_def_system(
        &self,
        a: &mut Self::State,
        node: &'ast DefSystem,
    ) -> ControlFlow<Self::Break> {
        self.check_spec_loc(a, SpecLocKind::System, node);
        ControlFlow::Continue(())
    }
}
