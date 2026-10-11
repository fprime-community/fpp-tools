use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{GenericNameSymbolMap, GenericScope, SymbolInterface};
use fpp_ast::{AstNode, Ident, QualIdent};
use fpp_core::Spanned;
use fpp_util::EnumMap;
use rustc_hash::FxHashMap as HashMap;
use std::fmt::Display;

/// Gets the scope of a symbol from the symbol scope map of an analysis, or
/// `None` if the symbol has no scope
pub type GetSymbolScope<A, NG, S, M> = for<'a> fn(&'a A, &S) -> Option<&'a GenericScope<NG, S, M>>;

/// Gets the use-def map of an analysis
pub type GetUseDefMap<A, S> = for<'a> fn(&'a mut A) -> &'a mut HashMap<fpp_core::Node, S>;

/// Generic helper functions for check uses
/// These are also used by the state machine analysis
///
/// `A` is the analysis state, `NG` the name group, `S` the symbol, and `M` the
/// name-symbol maps of a scope. The accessors are plain functions, so a
/// helper can be a constant.
pub struct CheckUsesHelpers<A, NG, S: SymbolInterface, M: EnumMap<NG, GenericNameSymbolMap<S>>> {
    /// Gets a relative or absolute symbol from the nested scope of the
    /// analysis: `(a, is_absolute, ng, name)`
    pub get_nested_scope_symbol: fn(&A, bool, NG, &str) -> Option<S>,
    /// Gets the scope of a symbol for resolving names in it
    pub get_symbol_scope: GetSymbolScope<A, NG, S, M>,
    /// Gets the use-def map
    pub get_use_def_map: GetUseDefMap<A, S>,
}

impl<A, NG, S, M> CheckUsesHelpers<A, NG, S, M>
where
    NG: Copy + Display,
    S: SymbolInterface,
    M: EnumMap<NG, GenericNameSymbolMap<S>>,
{
    /// Get the symbol for a name from the environment
    pub fn get_symbol_for_name(
        &self,
        ng: NG,
        mapping: impl Fn(&str) -> Option<S>,
        id: fpp_core::Node,
        name: &str,
    ) -> SemanticResult<S> {
        mapping(name).ok_or_else(|| SemanticError::UndefinedSymbol {
            ng: ng.to_string(),
            name: name.to_string(),
            loc: id.span(),
        })
    }

    /// Visit an identifier node and check a use
    pub fn visit_ident_node(
        &self,
        ng: NG,
        a: &mut A,
        node: &Ident,
        is_absolute: bool,
    ) -> SemanticResult {
        self.visit_unqualified_name(ng, a, node.id(), node, is_absolute)
            .map(|_| ())
    }

    /// Visit a qualified identifier node and check a use
    pub fn visit_qual_ident_node(&self, ng: NG, a: &mut A, node: &QualIdent) -> SemanticResult {
        let (symbol, name) = match node {
            QualIdent::Unqualified(unqualified) => (
                self.visit_unqualified_name(
                    ng,
                    a,
                    node.id(),
                    &unqualified.name,
                    unqualified.is_absolute,
                )?,
                &unqualified.name,
            ),
            QualIdent::Qualified(qualified) => (
                self.visit_qualified_name(ng, a, node.id(), &qualified.qualifier, &qualified.name)?,
                &qualified.name,
            ),
        };
        // The use is also recorded under the identifier node of the name, which
        // is where the language server looks for the use of a name
        (self.get_use_def_map)(a).insert(name.id(), symbol);
        Ok(())
    }

    fn visit_unqualified_name(
        &self,
        ng: NG,
        a: &mut A,
        id: fpp_core::Node,
        name: &Ident,
        is_absolute: bool,
    ) -> SemanticResult<S> {
        let mapping = |name: &str| (self.get_nested_scope_symbol)(a, is_absolute, ng, name);
        let symbol = self.get_symbol_for_name(ng, mapping, id, &name.data)?;
        (self.get_use_def_map)(a).insert(id, symbol.clone());
        Ok(symbol)
    }

    fn visit_qualified_name(
        &self,
        ng: NG,
        a: &mut A,
        id: fpp_core::Node,
        qualifier: &QualIdent,
        name: &Ident,
    ) -> SemanticResult<S> {
        self.visit_qual_ident_node(ng, a, qualifier)?;
        let symbol = {
            let qualifier_symbol = (self.get_use_def_map)(a)
                .get(&qualifier.id())
                .expect("qualifier should be in the use-def map")
                .clone();
            let scope = (self.get_symbol_scope)(a, &qualifier_symbol).ok_or_else(|| {
                SemanticError::InvalidSymbol {
                    symbol_name: qualifier_symbol.name().data.clone(),
                    msg: "not a qualifier".to_string(),
                    loc: id.span(),
                    def_loc: qualifier_symbol.get_loc(),
                }
            })?;
            let mapping = |name: &str| scope.get(ng, name);
            self.get_symbol_for_name(ng, mapping, name.id(), &name.data)?
        };
        (self.get_use_def_map)(a).insert(id, symbol.clone());
        Ok(symbol)
    }
}
