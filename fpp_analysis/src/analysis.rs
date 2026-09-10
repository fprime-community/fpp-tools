use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    FrameworkDefinitions, ImpliedUseSet, IntegerValue, Interface, NameGroup, NestedScope, Scope,
    Symbol, SymbolInterface, Type, UseDefMatching, Value,
};
use fpp_ast::{Expr, FormalParam, FormalParamKind, QueueFull, QueueFullSpecifier};
use fpp_core::{SourceFile, Span, Spanned};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::sync::Arc;

/// The analysis data structure
#[derive(Debug)]
pub struct Analysis {
    /// The mapping from symbols to their parent symbols
    pub parent_symbol_map: HashMap<Symbol, Symbol>,
    /// The outermost scope
    pub global_scope: Scope,
    /// The mapping from symbols with scopes to their scopes
    pub symbol_scope_map: HashMap<Symbol, Scope>,
    /// The mapping from definition node ID to their entered symbol
    pub symbol_map: HashMap<fpp_core::Node, Symbol>,
    /// The mapping from uses (by node ID) to their definitions
    pub use_def_map: HashMap<fpp_core::Node, Symbol>,
    /// The list of use-def matchings on the current use-def path.
    /// Used during cycle analysis.
    pub use_def_matching_list: Vec<UseDefMatching>,
    /// The set of symbols visited so far
    pub visited_symbol_set: HashSet<Symbol>,
    /// The set of symbols on the current use-def path.
    /// Used during cycle analysis.
    pub use_def_symbol_set: HashSet<Symbol>,
    /// The current parent symbol
    pub parent_symbol: Option<Symbol>,
    /// The current nested scope for symbol lookup
    pub nested_scope: NestedScope,
    /// The mapping from included files `.fppi` to their context they were included in
    pub include_context_map: HashMap<SourceFile, fpp_parser::IncludeParentKind>,
    /// The mapping from type and constant symbols, expressions,
    /// and type names to their types
    pub type_map: HashMap<fpp_core::Node, Arc<Type>>,
    /// The mapping from constant symbols and expressions to their values.
    pub value_map: HashMap<fpp_core::Node, Value>,
    /// The F Prime framework definitions found during analysis.
    pub framework_definitions: FrameworkDefinitions,
    /// The interface currently being analyzed.
    pub interface: Option<Interface>,
    /// The mapping from interface symbols to their resolved interfaces.
    pub interface_map: HashMap<Symbol, Interface>,
    /// The component currently being analyzed.
    pub component: Option<crate::semantics::Component>,
    /// The mapping from component symbols to their completed components.
    pub component_map: HashMap<Symbol, crate::semantics::Component>,
    /// The component instance currently being analyzed.
    pub component_instance: Option<crate::semantics::ComponentInstance>,
    /// The mapping from component instance symbols to their instances.
    pub component_instance_map: HashMap<Symbol, crate::semantics::ComponentInstance>,
    /// The topology currently being analyzed.
    pub topology: Option<crate::semantics::Topology>,
    /// The mapping from topology symbols to their partially resolved topologies.
    pub partial_topology_map: HashMap<Symbol, crate::semantics::Topology>,
    /// The mapping from topology symbols to their fully resolved topologies.
    pub topology_map: HashMap<Symbol, crate::semantics::Topology>,
    /// The mapping from system symbols to their resolved systems.
    pub system_map: HashMap<Symbol, crate::semantics::FppSystem>,
    /// The mapping from deployment topology symbols to their telemetry packet
    /// sets, by packet set name.
    pub tlm_packet_set_map: HashMap<Symbol, HashMap<String, crate::semantics::TlmPacketSet>>,
    /// The mapping from (location specifier kind, qualified name) to the
    /// location specifier that named it.
    pub location_specifier_map: HashMap<(fpp_ast::SpecLocKind, String), SpecLocEntry>,
    /// The list of enclosing scope (module) names on the current path.
    pub scope_name_list: Vec<String>,
    /// The mapping from state machine symbols to their analyzed state machines.
    pub state_machine_map: HashMap<Symbol, crate::semantics::state_machine::StateMachine>,
    /// The set of symbols marked as dictionary definitions and validated by
    /// `CheckDictionaryDefs`.
    pub dictionary_symbol_set: HashSet<Symbol>,

    /// Map from an AST node id to the implied uses of FPP symbols at that node.
    /// Populated by `ConstructImpliedUseMap` and consumed by the use-analysis
    /// passes (`CheckUses`, `CheckUseDefCycles`).
    pub implied_use_map: HashMap<fpp_core::Node, ImpliedUseSet>,

    /// The shared "unknown" type standing in for a type use that failed to
    /// resolve. Created lazily on first use so that `CheckTypeUses` can give
    /// every type name an entry in `type_map`, letting later passes read a
    /// resolved type without handling the unresolved case.
    unknown_type: Option<Arc<Type>>,
}

/// A type definition that [`crate::passes::EnterSymbols`] interns into a
/// [`Symbol`].
///
/// The semantic [`Type`] for a type definition holds the definition behind the
/// same `Arc` that the symbol table holds, so the symbol and the type share one
/// definition node.
pub trait InternedDef: fpp_ast::AstNode + Clone {
    /// Borrows the interned definition out of `symbol`, if `symbol` names one of
    /// this kind.
    fn from_symbol(symbol: &Symbol) -> Option<&Arc<Self>>;
}

macro_rules! impl_interned_def {
    ($($ty:ident => $variant:ident),* $(,)?) => {
        $(impl InternedDef for fpp_ast::$ty {
            fn from_symbol(symbol: &Symbol) -> Option<&Arc<Self>> {
                match symbol {
                    Symbol::$variant(def) => Some(def),
                    _ => None,
                }
            }
        })*
    };
}

impl_interned_def! {
    DefAbsType => AbsType,
    DefAliasType => AliasType,
    DefArray => Array,
    DefEnum => Enum,
    DefStruct => Struct,
}

/// A recorded location specifier, keyed in `location_specifier_map`.
#[derive(Debug, Clone)]
pub struct SpecLocEntry {
    /// Span of the location specifier statement
    pub spec_span: Span,
    /// Span of the file string literal (error location + base for path resolution)
    pub file_span: Span,
    /// The specified (relative) path string
    pub file_value: String,
    /// Whether this is a dictionary specifier
    pub is_dictionary_def: bool,
}

impl Default for Analysis {
    fn default() -> Self {
        Self::new()
    }
}

impl Analysis {
    pub fn new() -> Analysis {
        // Validate that Analysis is thread safe
        fn is_sync<T: Sync>() {}
        is_sync::<Analysis>();

        Analysis {
            parent_symbol_map: Default::default(),
            global_scope: Scope::new(),
            symbol_scope_map: Default::default(),
            symbol_map: Default::default(),
            use_def_map: Default::default(),
            use_def_matching_list: vec![],
            visited_symbol_set: Default::default(),
            use_def_symbol_set: Default::default(),
            parent_symbol: None,
            nested_scope: NestedScope::new(),
            include_context_map: Default::default(),
            type_map: Default::default(),
            value_map: Default::default(),
            framework_definitions: Default::default(),
            interface: None,
            interface_map: Default::default(),
            component: None,
            component_map: Default::default(),
            component_instance: None,
            component_instance_map: Default::default(),
            topology: None,
            partial_topology_map: Default::default(),
            topology_map: Default::default(),
            system_map: Default::default(),
            tlm_packet_set_map: Default::default(),
            location_specifier_map: Default::default(),
            scope_name_list: Vec::new(),
            state_machine_map: Default::default(),
            dictionary_symbol_set: Default::default(),
            implied_use_map: Default::default(),
            unknown_type: None,
        }
    }

    /// The shared "unknown" type used when a type use fails to resolve.
    ///
    /// This is a synthetic abstract type whose name is not valid FPP syntax, so
    /// it can never collide with a user-defined type. It is created once and
    /// reused, so every unknown type is [`Type::identical`] to every other,
    /// which keeps a single unresolved use from cascading into spurious type
    /// mismatches downstream. The `span` seeds the synthetic definition node the
    /// first time the type is created; it is never rendered (the type only ever
    /// surfaces through its name), so any use-site span will do.
    pub fn unknown_type(&mut self, span: Span) -> Arc<Type> {
        if let Some(ty) = &self.unknown_type {
            return ty.clone();
        }
        let node_id = fpp_core::Node::new(span);
        let ty = Arc::new(Type::AbsType(crate::semantics::AbsType {
            node: Arc::new(fpp_ast::DefAbsType {
                node_id,
                name: fpp_ast::Name {
                    node_id,
                    data: "<unknown>".to_string(),
                },
            }),
        }));
        self.unknown_type = Some(ty.clone());
        ty
    }

    /// The `Arc` holding a type definition, for storing in its semantic
    /// [`Type`].
    ///
    /// `EnterSymbols` already interned an `Arc` of the definition in
    /// `symbol_map`, so reuse that allocation instead of deep-copying the AST:
    /// `Type` is `Clone` and cloned freely, and `Type::def_symbol` hands the
    /// `Arc` straight back out as a `Symbol`. The clone fallback covers
    /// definitions that no symbol was entered for (there are none in the pass
    /// pipeline, but a synthesized definition need not be in `symbol_map`).
    pub fn interned_def<D: InternedDef>(&self, node: &D) -> Arc<D> {
        match self.symbol_map.get(&node.id()).and_then(D::from_symbol) {
            Some(def) => def.clone(),
            None => Arc::new(node.clone()),
        }
    }

    /// Gets the finalized type of a type-name use node.
    ///
    /// `FinalizeTypeDefs` rewrites the type recorded for a type DEFINITION
    /// node; the type names that use it keep the type they were given by
    /// `CheckTypeUses`, which may not be finalized yet (an array whose size is
    /// not yet known, a struct with no default value). Resolve the use through
    /// its definition node, so callers always see the finalized type.
    pub fn get_finalized_type(&self, node: fpp_core::Node) -> Option<Arc<Type>> {
        let ty = self.type_map.get(&node)?.clone();
        match ty.def_node_id() {
            Some(def_node) => Some(self.type_map.get(&def_node).cloned().unwrap_or(ty)),
            None => Some(ty),
        }
    }

    /// Get an integer value for an AST node from the value map, if present.
    pub fn get_int_value(&self, node: fpp_core::Node) -> Option<i128> {
        match self
            .value_map
            .get(&node)
            .and_then(|v| v.convert(&Arc::new(Type::Integer)))
        {
            Some(Value::Integer(IntegerValue(v))) => Some(v),
            _ => None,
        }
    }

    /// Get an optional integer value for an optional expression.
    pub fn get_big_int_value_opt(&self, expr: &Option<Expr>) -> Option<i128> {
        expr.as_ref().and_then(|e| self.get_int_value(e.node_id))
    }

    /// Gets an int value from an AST node, erroring if it is out of the i32
    /// range. `phase`/id values are `Int`-typed; an FPP arbitrary-precision
    /// integer that overflows `Int` is rejected with "value out of range".
    pub fn get_int_value_checked(&self, node: fpp_core::Node, loc: Span) -> SemanticResult<i128> {
        let v = self.get_int_value(node).unwrap_or(0);
        if v < i32::MIN as i128 || v > i32::MAX as i128 {
            Err(SemanticError::InvalidIntValue {
                loc,
                v: Some(v),
                msg: "value out of range".to_string(),
            })
        } else {
            Ok(v)
        }
    }

    /// Get an array size (>= 1) for an AST node.
    pub fn get_array_size(
        &self,
        node: fpp_core::Node,
        loc: fpp_core::Span,
    ) -> SemanticResult<i128> {
        let v = self.get_int_value(node).unwrap_or(1);
        if v >= 1 {
            Ok(v)
        } else {
            Err(SemanticError::InvalidArraySize { loc, size: v })
        }
    }

    /// Get an optional array size, defaulting to 1 when the expression is absent.
    pub fn get_array_size_opt(&self, expr: &Option<Expr>) -> SemanticResult<i128> {
        match expr {
            Some(e) => self.get_array_size(e.node_id, e.span()),
            None => Ok(1),
        }
    }

    /// Get a nonnegative integer value for an AST node.
    pub fn get_nonnegative_big_int_value(
        &self,
        node: fpp_core::Node,
        loc: Span,
    ) -> SemanticResult<i128> {
        let v = self.get_int_value(node).unwrap_or(0);
        if v >= 0 {
            Ok(v)
        } else {
            Err(SemanticError::InvalidIntValue {
                loc,
                v: Some(v),
                msg: "value may not be negative".to_string(),
            })
        }
    }

    /// Get an optional nonnegative integer value for an optional expression.
    pub fn get_nonnegative_big_int_value_opt(
        &self,
        expr: &Option<Expr>,
    ) -> SemanticResult<Option<i128>> {
        match expr {
            Some(e) => Ok(Some(
                self.get_nonnegative_big_int_value(e.node_id, e.span())?,
            )),
            None => Ok(None),
        }
    }

    /// Get a nonnegative int value (in i32 range) for an AST node.
    pub fn get_nonnegative_int_value(
        &self,
        node: fpp_core::Node,
        loc: Span,
    ) -> SemanticResult<i128> {
        let v = self.get_int_value(node).unwrap_or(0);
        if v < i32::MIN as i128 || v > i32::MAX as i128 {
            return Err(SemanticError::InvalidIntValue {
                loc,
                v: Some(v),
                msg: "value out of range".to_string(),
            });
        }
        if v >= 0 {
            Ok(v)
        } else {
            Err(SemanticError::InvalidIntValue {
                loc,
                v: Some(v),
                msg: "value may not be negative".to_string(),
            })
        }
    }

    /// Get a queue full behavior, defaulting to `Assert`.
    pub fn get_queue_full(opt: &Option<QueueFull>) -> QueueFull {
        opt.clone().unwrap_or(QueueFull::Assert)
    }

    /// Get the queue full behavior named by an optional queue full specifier,
    /// defaulting to `Assert`.
    pub fn get_specified_queue_full(opt: &Option<QueueFullSpecifier>) -> QueueFull {
        Self::get_queue_full(&opt.as_ref().map(|spec| spec.kind.clone()))
    }

    /// Count the number of ref parameters in a formal parameter list.
    pub fn get_num_ref_params(params: &[FormalParam]) -> usize {
        params
            .iter()
            .filter(|p| matches!(p.kind, FormalParamKind::Ref))
            .count()
    }

    /// Check that a formal parameter list has no duplicate parameter names.
    pub fn check_for_duplicate_parameter(params: &[FormalParam]) -> SemanticResult {
        let mut seen: HashMap<String, Span> = HashMap::default();
        for param in params {
            if let Some(prev_loc) = seen.insert(param.name.data.clone(), param.name.span()) {
                return Err(SemanticError::DuplicateParameter {
                    name: param.name.data.clone(),
                    loc: param.name.span(),
                    prev_loc,
                });
            }
        }
        Ok(())
    }

    /// Check that the type of an AST node is displayable.
    pub fn check_displayable_type(
        &self,
        node: fpp_core::Node,
        loc: Span,
        msg: &str,
    ) -> SemanticResult {
        match self.type_map.get(&node) {
            Some(ty) if ty.is_displayable() => Ok(()),
            Some(_) => Err(SemanticError::InvalidType {
                loc,
                msg: msg.to_string(),
            }),
            None => Ok(()),
        }
    }

    /// Check that the types of all formal parameters are displayable.
    pub fn check_displayable_params(&self, params: &[FormalParam], msg: &str) -> SemanticResult {
        for param in params {
            self.check_displayable_type(param.type_name.node_id, param.type_name.span(), msg)?;
        }
        Ok(())
    }

    /// Compute the fully qualified name of a symbol by walking up the
    /// parent-symbol map, joining component names with `.`.
    pub fn get_qualified_name(&self, symbol: &Symbol) -> String {
        let mut parts = vec![symbol.name().data.clone()];
        let mut current = symbol.clone();
        while let Some(parent) = self.parent_symbol_map.get(&current) {
            parts.push(parent.name().data.clone());
            current = parent.clone();
        }
        parts.reverse();
        parts.join(".")
    }

    pub fn get_symbol<N: fpp_ast::AstNode>(&self, node: &N) -> Symbol {
        self.symbol_map.get(&node.id()).unwrap().clone()
    }

    /// Resolve a use (by node ID) to a topology symbol. Returns `Ok(None)` for
    /// an unresolved use (already reported by `CheckUses`) and an error if the
    /// use resolves to a non-topology symbol.
    pub fn get_topology_symbol(&self, id: fpp_core::Node) -> SemanticResult<Option<Symbol>> {
        match self.use_def_map.get(&id) {
            Some(symbol @ Symbol::Topology(_)) => Ok(Some(symbol.clone())),
            Some(symbol) => Err(SemanticError::InvalidSymbol {
                symbol_name: symbol.name().data.clone(),
                msg: format!(
                    "invalid use of symbol {}: not a topology symbol",
                    symbol.name().data
                ),
                loc: id.span(),
                def_loc: symbol.node().span(),
            }),
            // Unresolved use: CheckUses already reported the error.
            None => Ok(None),
        }
    }

    pub fn get_scope(&self, symbol: &Option<Symbol>) -> &Scope {
        match symbol {
            None => &self.global_scope,
            Some(s) => self
                .symbol_scope_map
                .get(s)
                .unwrap_or_else(|| panic!("symbol {} does not have a scope", s.name().data)),
        }
    }

    pub fn symbol_get(&self, name_group: NameGroup, name: &str) -> Option<Symbol> {
        self.nested_scope
            .search(|s| self.get_scope(s).get(name_group, name))
    }

    pub fn get_scope_mut(&mut self, symbol: &Option<Symbol>) -> &mut Scope {
        match symbol {
            None => &mut self.global_scope,
            Some(s) => self
                .symbol_scope_map
                .get_mut(s)
                .unwrap_or_else(|| panic!("symbol {} does not have a scope", s.name().data)),
        }
    }

    pub fn symbol_put(&mut self, name_group: NameGroup, symbol: Symbol) -> SemanticResult {
        let scope = self.nested_scope.current().clone();
        self.get_scope_mut(&scope).put(name_group, symbol)
    }
}
