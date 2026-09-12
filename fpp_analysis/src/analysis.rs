use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    Component, ComponentInstance, Dictionary, FppSystem, FrameworkDefinitions, ImpliedUse,
    ImpliedUseKind, ImpliedUseSet, IntegerValue, Interface, NameGroup, NestedScope, QualifiedName,
    Scope, Symbol, SymbolInterface, TlmPacketSet, Topology, Type, UseDefMatching, Value,
    state_machine::StateMachine,
};
use fpp_ast::{Expr, FormalParam, FormalParamKind, QueueFull, QueueFullSpecifier, SpecLoc};
use fpp_core::{File, SourceFile, Span, Spanned};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::sync::Arc;

/// Whether to keep or omit a component prefix when computing a short name
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentPrefix {
    Keep,
    Omit,
}

/// The analysis data structure
#[derive(Debug)]
pub struct Analysis {
    /// The set of files presented to the analyzer
    pub input_file_set: HashSet<File>,
    /// The recursive level of the analysis
    pub(crate) level: i32,
    /// The set of files on which the analysis transitively depends.
    /// Does not contain included files.
    pub dependency_file_set: HashSet<File>,
    /// The set of files on which the analysis directly depends.
    /// Does contain included files.
    pub direct_dependency_file_set: HashSet<File>,
    /// The set of dependency files that could not be opened
    pub missing_dependency_file_set: HashSet<File>,
    /// The set of files included when parsing input
    pub included_file_set: HashSet<File>,
    /// The mapping from included files `.fppi` to the context they were included in
    pub include_context_map: HashMap<SourceFile, fpp_parser::IncludeParentKind>,
    /// A map from pairs (spec loc kind, qualified name) to spec locs.
    pub location_specifier_map: HashMap<(fpp_ast::SpecLocKind, QualifiedName), Arc<SpecLoc>>,
    /// A list of unqualified names representing the enclosing scope names,
    /// with the innermost name at the head of the list. For example, inside
    /// module B where B is inside A and A is at the top level, the module name
    /// list is [ B, A ].
    pub(crate) scope_name_list: Vec<String>,
    /// Whether dictionary generation is required
    pub dictionary_generation: bool,
    /// Whether the dependency analysis includes dictionary dependencies
    pub(crate) include_dictionary_deps: bool,
    /// The current nested scope for symbol lookup
    pub(crate) nested_scope: NestedScope,
    /// The outermost scope
    pub global_scope: Scope,
    /// The current parent symbol
    pub(crate) parent_symbol: Option<Symbol>,
    /// The mapping from symbols to their parent symbols
    pub parent_symbol_map: HashMap<Symbol, Symbol>,
    /// The mapping from symbols with scopes to their scopes
    pub symbol_scope_map: HashMap<Symbol, Scope>,
    /// The mapping from uses (by node ID) to their definitions
    pub use_def_map: HashMap<fpp_core::Node, Symbol>,
    /// The mapping from definition node ID to their entered symbol
    pub symbol_map: HashMap<fpp_core::Node, Symbol>,
    /// The set of symbols visited so far
    pub(crate) visited_symbol_set: HashSet<Symbol>,
    /// The set of symbols on the current use-def path.
    /// Used during cycle analysis.
    pub(crate) use_def_symbol_set: HashSet<Symbol>,
    /// The list of use-def matchings on the current use-def path.
    /// Used during cycle analysis.
    pub(crate) use_def_matching_list: Vec<UseDefMatching>,
    /// The mapping from type and constant symbols, expressions,
    /// and type names to their types
    pub type_map: HashMap<fpp_core::Node, Arc<Type>>,
    /// The mapping from constant symbols and expressions to their values.
    pub value_map: HashMap<fpp_core::Node, Value>,
    /// The set of symbols used. Used during code generation.
    pub(crate) used_symbol_set: HashSet<Symbol>,
    /// The framework definitions present in the model
    pub framework_definitions: FrameworkDefinitions,
    /// The map from component symbols to components
    pub component_map: HashMap<Symbol, Component>,
    /// The component under construction
    pub(crate) component: Option<Component>,
    /// The map from component instance symbols to component instances
    pub component_instance_map: HashMap<Symbol, ComponentInstance>,
    /// The component instance under construction
    pub(crate) component_instance: Option<ComponentInstance>,
    /// The map from interface symbols to interfaces
    pub interface_map: HashMap<Symbol, Interface>,
    /// The interface under construction
    pub(crate) interface: Option<Interface>,
    /// The map from topology symbols to 'partial' topologies
    /// with only port interface/instance information
    pub partial_topology_map: HashMap<Symbol, Topology>,
    /// The map from topology symbols to topologies
    pub topology_map: HashMap<Symbol, Topology>,
    /// The topology under construction
    pub(crate) topology: Option<Topology>,
    /// The map from state machine symbols to state machines
    pub state_machine_map: HashMap<Symbol, StateMachine>,
    /// The map from topology symbols to dictionaries
    pub dictionary_map: HashMap<Symbol, Dictionary>,
    /// The dictionary under construction
    pub(crate) dictionary: Option<Dictionary>,
    /// The telemetry packet set under construction
    pub(crate) tlm_packet_set: Option<TlmPacketSet>,
    /// The mapping from nodes to implied uses
    pub implied_use_map: HashMap<fpp_core::Node, ImpliedUseSet>,
    /// The set of symbols defined with a dictionary specifier
    pub dictionary_symbol_set: HashSet<Symbol>,
    /// The mapping from system symbols to systems
    pub system_map: HashMap<Symbol, FppSystem>,

    /// The shared "unknown" type standing in for a type use that failed to
    /// resolve. Created lazily on first use so that `CheckTypeUses` can give
    /// every type name an entry in `type_map`, letting later passes read a
    /// resolved type without handling the unresolved case.
    unknown_type: Option<Arc<Type>>,
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
            input_file_set: Default::default(),
            level: 0,
            dependency_file_set: Default::default(),
            direct_dependency_file_set: Default::default(),
            missing_dependency_file_set: Default::default(),
            included_file_set: Default::default(),
            include_context_map: Default::default(),
            location_specifier_map: Default::default(),
            scope_name_list: Vec::new(),
            dictionary_generation: false,
            include_dictionary_deps: false,
            nested_scope: NestedScope::new(),
            global_scope: Scope::new(),
            parent_symbol: None,
            parent_symbol_map: Default::default(),
            symbol_scope_map: Default::default(),
            use_def_map: Default::default(),
            symbol_map: Default::default(),
            visited_symbol_set: Default::default(),
            use_def_symbol_set: Default::default(),
            use_def_matching_list: vec![],
            type_map: Default::default(),
            value_map: Default::default(),
            used_symbol_set: Default::default(),
            framework_definitions: Default::default(),
            component_map: Default::default(),
            component: None,
            component_instance_map: Default::default(),
            component_instance: None,
            interface_map: Default::default(),
            interface: None,
            partial_topology_map: Default::default(),
            topology_map: Default::default(),
            topology: None,
            state_machine_map: Default::default(),
            dictionary_map: Default::default(),
            dictionary: None,
            tlm_packet_set: None,
            implied_use_map: Default::default(),
            dictionary_symbol_set: Default::default(),
            system_map: Default::default(),
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
        let ty = Arc::new(Type::Abs(Arc::new(crate::semantics::AbsType {
            node: Arc::new(fpp_ast::DefAbsType {
                node_id,
                name: fpp_ast::Name {
                    node_id,
                    data: "<unknown>".to_string(),
                },
            }),
        })));
        self.unknown_type = Some(ty.clone());
        ty
    }

    /// Gets the qualified name of a symbol
    pub fn get_qualified_name(&self, symbol: &Symbol) -> String {
        Analysis::get_qualified_name_from_map(&self.parent_symbol_map, symbol).to_string()
    }

    /// Gets the qualified name of a symbol from a parent-symbol map
    pub fn get_qualified_name_from_map(
        parent_symbol_map: &HashMap<Symbol, Symbol>,
        symbol: &Symbol,
    ) -> QualifiedName {
        let mut parts = vec![symbol.name().data.clone()];
        let mut current = symbol.clone();
        while let Some(parent) = parent_symbol_map.get(&current) {
            parts.push(parent.name().data.clone());
            current = parent.clone();
        }
        parts.reverse();
        parts.into()
    }

    /// Gets the short name of a symbol
    /// When generating C++, we may need to keep the component prefix, because
    /// it is part of the symbol name
    pub fn get_short_name(
        &self,
        symbol: &Symbol,
        context: &Symbol,
        component_prefix: ComponentPrefix,
    ) -> QualifiedName {
        let name = Analysis::get_qualified_name_from_map(&self.parent_symbol_map, symbol);
        let mut prefix = self.get_enclosing_names(context);
        match (context, component_prefix) {
            (Symbol::Component(_), ComponentPrefix::Keep) => {}
            _ => prefix.push(context.name().data.clone()),
        }
        name.short_name(&prefix)
    }

    /// Gets the list of enclosing identifiers for a symbol
    pub fn get_enclosing_names(&self, symbol: &Symbol) -> Vec<String> {
        match self.parent_symbol_map.get(symbol) {
            Some(parent) => Analysis::get_qualified_name_from_map(&self.parent_symbol_map, parent)
                .to_ident_list()
                .into(),
            None => Vec::new(),
        }
    }

    /// Add a mapping to the type map
    pub fn assign_type(&mut self, node: fpp_core::Node, ty: Arc<Type>) {
        self.type_map.insert(node, ty);
    }

    /// Add a value to the value map
    pub fn assign_value(&mut self, node: fpp_core::Node, value: Value) {
        self.value_map.insert(node, value);
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

    /// Gets a component from the component map
    pub fn get_component(&self, id: fpp_core::Node) -> SemanticResult<Option<Component>> {
        match self.use_def_map.get(&id) {
            Some(symbol @ Symbol::Component(_)) => Ok(self.component_map.get(symbol).cloned()),
            Some(symbol) => Err(Analysis::invalid_symbol(
                symbol,
                id,
                "not a component symbol",
            )),
            None => Ok(None),
        }
    }

    /// Gets a component instance symbol from the use-def map
    pub fn get_component_instance_symbol(
        &self,
        id: fpp_core::Node,
    ) -> SemanticResult<Option<Symbol>> {
        match self.use_def_map.get(&id) {
            Some(symbol @ Symbol::ComponentInstance(_)) => Ok(Some(symbol.clone())),
            Some(symbol) => Err(Analysis::invalid_symbol(
                symbol,
                id,
                "not a component instance symbol",
            )),
            None => Ok(None),
        }
    }

    /// Gets an interface instance symbol from the use-def map
    pub fn get_interface_instance_symbol(
        &self,
        id: fpp_core::Node,
    ) -> SemanticResult<Option<Symbol>> {
        match self.use_def_map.get(&id) {
            Some(symbol @ (Symbol::ComponentInstance(_) | Symbol::Topology(_))) => {
                Ok(Some(symbol.clone()))
            }
            Some(symbol) => Err(Analysis::invalid_symbol(
                symbol,
                id,
                "not a component instance or topology symbol",
            )),
            None => Ok(None),
        }
    }

    /// Gets an interface symbol from the use-def map
    pub fn get_interface_symbol(&self, id: fpp_core::Node) -> SemanticResult<Option<Symbol>> {
        match self.use_def_map.get(&id) {
            Some(symbol @ Symbol::Interface(_)) => Ok(Some(symbol.clone())),
            Some(symbol) => Err(Analysis::invalid_symbol(
                symbol,
                id,
                "not a interface symbol",
            )),
            None => Ok(None),
        }
    }

    /// Gets a topology symbol from the use-def map
    pub fn get_topology_symbol(&self, id: fpp_core::Node) -> SemanticResult<Option<Symbol>> {
        match self.use_def_map.get(&id) {
            Some(symbol @ Symbol::Topology(_)) => Ok(Some(symbol.clone())),
            Some(symbol) => Err(Analysis::invalid_symbol(
                symbol,
                id,
                "not a topology symbol",
            )),
            None => Ok(None),
        }
    }

    /// Gets a topology from the topology map
    pub fn get_topology(&self, id: fpp_core::Node) -> SemanticResult<Option<Topology>> {
        match self.get_topology_symbol(id)? {
            Some(symbol) => Ok(self.topology_map.get(&symbol).cloned()),
            None => Ok(None),
        }
    }

    /// Gets a dictionary from the dictionary map
    pub fn get_dictionary(&self, id: fpp_core::Node) -> SemanticResult<Option<Dictionary>> {
        match self.get_topology_symbol(id)? {
            Some(symbol) => Ok(self.dictionary_map.get(&symbol).cloned()),
            None => Ok(None),
        }
    }

    fn invalid_symbol(symbol: &Symbol, id: fpp_core::Node, msg: &str) -> SemanticError {
        SemanticError::InvalidSymbol {
            symbol_name: symbol.name().data.clone(),
            msg: format!("invalid use of symbol {}: {}", symbol.name().data, msg),
            loc: id.span(),
            def_loc: symbol.node().span(),
        }
    }

    /// Gets the implied uses for an AST node
    pub fn get_implied_uses(&self, kind: ImpliedUseKind, id: fpp_core::Node) -> Vec<ImpliedUse> {
        match self.implied_use_map.get(&id) {
            Some(uses) => uses.get(kind).to_vec(),
            None => Vec::new(),
        }
    }

    /// Gets an integer value from an AST node
    pub fn get_big_int_value(&self, node: fpp_core::Node) -> Option<i128> {
        self.get_int_value(node)
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

    /// Displays an ID value
    pub fn display_id_value(value: i128) -> String {
        format!("({} dec, {:X} hex)", value, value)
    }

    /// Adds a dictionary element mapped by ID, returning the new default ID
    pub fn add_element_to_id_map<T, M: IdMap<T>>(
        map: &mut M,
        id: i128,
        element: T,
        get_loc: impl Fn(&T) -> Span,
    ) -> SemanticResult<i128> {
        match map.get(&id) {
            Some(prev) => Err(SemanticError::DuplicateIdValue {
                value: Analysis::display_id_value(id),
                loc: get_loc(&element),
                prev_loc: get_loc(prev),
            }),
            None => {
                map.insert(id, element);
                Ok(id + 1)
            }
        }
    }

    /// Checks for duplicate names in a dictionary
    pub fn check_dictionary_names<T, M: IdMap<T>>(
        dictionary: &M,
        kind: &str,
        get_name: impl Fn(&T) -> String,
        get_loc: impl Fn(&T) -> Span,
    ) -> SemanticResult {
        let mut seen: HashMap<String, Span> = HashMap::default();
        for value in dictionary.values_by_id() {
            let name = get_name(value);
            let loc = get_loc(value);
            if let Some(prev_loc) = seen.insert(name.clone(), loc) {
                return Err(SemanticError::DuplicateDictionaryName {
                    kind: kind.to_string(),
                    name,
                    loc,
                    prev_loc,
                });
            }
        }
        Ok(())
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

    /// Gets the reason for a non-displayable type at an AST node id, as the
    /// chain of locations that leads from the node to the definition that is not
    /// displayable.
    pub fn get_reason_for_non_displayable_type_at(
        &self,
        node: fpp_core::Node,
    ) -> Vec<(Span, String)> {
        let element_reason = |node: fpp_core::Node| {
            let mut reason = vec![(
                node.span(),
                "because this type is not displayable".to_string(),
            )];
            reason.extend(self.get_reason_for_non_displayable_type_at(node));
            reason
        };
        match self.type_map.get(&node).map(|t| t.as_ref()) {
            Some(Type::Alias(ty)) => element_reason(ty.node.type_name.node_id),
            Some(Type::Array(ty)) => element_reason(ty.node.elt_type.node_id),
            Some(Type::Struct(ty)) => {
                match ty.node.members.iter().find(|m| {
                    !self
                        .type_map
                        .get(&m.type_name.node_id)
                        .is_none_or(|t| t.is_displayable())
                }) {
                    Some(member) => element_reason(member.type_name.node_id),
                    None => Vec::new(),
                }
            }
            Some(ty) => match ty.def_node_id() {
                Some(def_node) => vec![(def_node.span(), "type is defined here".to_string())],
                None => Vec::new(),
            },
            None => Vec::new(),
        }
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
                notes: self.get_reason_for_non_displayable_type_at(node),
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

    /// The symbol that [`crate::passes::EnterSymbols`] entered for a definition
    /// node.
    pub fn get_symbol<N: fpp_ast::AstNode>(&self, node: &N) -> Option<Symbol> {
        self.symbol_map.get(&node.id()).cloned()
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

/// An ID-keyed dictionary map that [`Analysis::add_element_to_id_map`] can fill
pub trait IdMap<T> {
    fn get(&self, id: &i128) -> Option<&T>;
    fn insert(&mut self, id: i128, element: T);
    /// The values of the map, in id order
    fn values_by_id(&self) -> Vec<&T>;
}

impl<T> IdMap<T> for HashMap<i128, T> {
    fn get(&self, id: &i128) -> Option<&T> {
        HashMap::get(self, id)
    }

    fn insert(&mut self, id: i128, element: T) {
        HashMap::insert(self, id, element);
    }

    fn values_by_id(&self) -> Vec<&T> {
        let mut ids: Vec<&i128> = self.keys().collect();
        ids.sort();
        ids.into_iter().map(|id| &self[id]).collect()
    }
}

impl<T> IdMap<T> for std::collections::BTreeMap<i128, T> {
    fn get(&self, id: &i128) -> Option<&T> {
        std::collections::BTreeMap::get(self, id)
    }

    fn insert(&mut self, id: i128, element: T) {
        std::collections::BTreeMap::insert(self, id, element);
    }

    fn values_by_id(&self) -> Vec<&T> {
        self.values().collect()
    }
}
