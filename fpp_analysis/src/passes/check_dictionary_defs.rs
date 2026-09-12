use crate::Analysis;
use crate::semantics::{Symbol, Type};
use fpp_ast::{
    DefAliasType, DefArray, DefConstant, DefEnum, DefStruct, MoveWalkable, Node, Visitor,
};
use fpp_core::Spanned;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Check dictionary definitions.
///
/// For each definition marked as a dictionary definition (`dictionary`
/// prefix), it validates:
///
/// - A dictionary constant must have a numeric, Boolean, string, or enum type.
/// - A dictionary type definition must be displayable.
///
/// Validated symbols are recorded in `Analysis::dictionary_symbol_set`.
pub struct CheckDictionaryDefs;

impl CheckDictionaryDefs {
    fn check_constant_def(a: &mut Analysis, node: &DefConstant) {
        if !node.is_dictionary_def {
            return;
        }
        let symbol = Symbol::Constant(Arc::new(node.clone()));
        let ok = match a.type_map.get(&node.node_id).map(|t| t.as_ref()) {
            Some(Type::String(_)) | Some(Type::Boolean) | Some(Type::Enum(_)) => true,
            Some(t) => t.is_numeric(),
            // No recorded type: be lenient (an earlier pass already reported an error).
            None => true,
        };
        if ok {
            a.dictionary_symbol_set.insert(symbol);
        } else {
            crate::errors::SemanticError::InvalidType {
                loc: node.span(),
                msg: "dictionary constant must have a numeric, Boolean, string, or enum type"
                    .to_string(),
                notes: vec![],
            }
            .emit();
        }
    }

    fn check_type_def(a: &mut Analysis, symbol: Symbol, node: fpp_core::Node, loc: fpp_core::Span) {
        if !symbol.is_dictionary_def() {
            return;
        }
        match a.check_displayable_type(node, loc, "dictionary type is not displayable") {
            Ok(()) => {
                a.dictionary_symbol_set.insert(symbol);
            }
            Err(err) => err.emit(),
        }
    }
}

impl<'ast> Visitor<'ast> for CheckDictionaryDefs {
    type Break = ();
    type State = Analysis;

    /// Descend into every container so that dictionary definitions nested in
    /// modules and components are reached.
    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        node.walk(a, self)
    }

    fn visit_def_constant(
        &self,
        a: &mut Analysis,
        node: &'ast DefConstant,
    ) -> ControlFlow<Self::Break> {
        Self::check_constant_def(a, node);
        ControlFlow::Continue(())
    }

    fn visit_def_alias_type(
        &self,
        a: &mut Analysis,
        node: &'ast DefAliasType,
    ) -> ControlFlow<Self::Break> {
        Self::check_type_def(
            a,
            Symbol::AliasType(Arc::new(node.clone())),
            node.node_id,
            node.span(),
        );
        ControlFlow::Continue(())
    }

    fn visit_def_array(&self, a: &mut Analysis, node: &'ast DefArray) -> ControlFlow<Self::Break> {
        Self::check_type_def(
            a,
            Symbol::ArrayType(Arc::new(node.clone())),
            node.node_id,
            node.span(),
        );
        ControlFlow::Continue(())
    }

    fn visit_def_enum(&self, a: &mut Analysis, node: &'ast DefEnum) -> ControlFlow<Self::Break> {
        Self::check_type_def(
            a,
            Symbol::EnumType(Arc::new(node.clone())),
            node.node_id,
            node.span(),
        );
        ControlFlow::Continue(())
    }

    fn visit_def_struct(
        &self,
        a: &mut Analysis,
        node: &'ast DefStruct,
    ) -> ControlFlow<Self::Break> {
        Self::check_type_def(
            a,
            Symbol::StructType(Arc::new(node.clone())),
            node.node_id,
            node.span(),
        );
        ControlFlow::Continue(())
    }
}
