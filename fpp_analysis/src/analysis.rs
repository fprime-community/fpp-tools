use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    Component, ComponentInstance, Dictionary, Direction, FppSystem, FrameworkDefinitions,
    ImpliedUse, ImpliedUseKind, ImpliedUseSet, IntegerValue, Interface, InterfaceInstance,
    MathError, MathResult, NameGroup, NestedScope, PortInstance, PortInstanceType, QualifiedName,
    Scope, ScopeOwner, Symbol, SymbolInterface, TemplateExpansion, TlmPacketSet, Topology, Type,
    TypeConversionError, UseDefMatching, Value, state_machine::StateMachine,
};
use fpp_ast::{AstNode, Expr, FormalParam, FormalParamKind, QueueFull, SpecLoc};
use fpp_core::{File, SourceFile, Span, Spanned};
use indexmap::IndexMap;
use rustc_hash::{FxBuildHasher, FxHashMap as HashMap, FxHashSet as HashSet};
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
    /// The map from component instance symbols to component instances, in
    /// the order the instances were checked
    pub component_instance_map: indexmap::IndexMap<Symbol, ComponentInstance, FxBuildHasher>,
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
    /// The mapping from template expansion specifier nodes to their expansions,
    /// in the order the expansions are entered
    pub template_expansion_map: IndexMap<fpp_core::Node, TemplateExpansion>,
    /// The node of the template expansion specifier whose expansion we are
    /// currently analyzing inside of
    pub(crate) template_expansion: Option<fpp_core::Node>,
    /// The set of symbols defined with a dictionary specifier
    pub dictionary_symbol_set: HashSet<Symbol>,
    /// The mapping from system symbols to systems
    pub system_map: HashMap<Symbol, FppSystem>,

    /// The shared "unknown" type of a type name that does not resolve to a
    /// type. Created lazily on first use so that `CheckTypeUses` can give
    /// every type name an entry in `type_map`, letting later passes read a
    /// type without handling the unresolved case.
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
            nested_scope: NestedScope::empty(),
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
            template_expansion_map: Default::default(),
            template_expansion: None,
            dictionary_symbol_set: Default::default(),
            system_map: Default::default(),
            unknown_type: None,
        }
    }

    /// The shared "unknown" type used when a type use fails to resolve.
    ///
    /// The use has been reported as an error; the unknown type gives its type
    /// name an entry in the type map so the analysis can go on. This is a
    /// synthetic abstract type whose name, `<unknown>`, is not valid FPP
    /// syntax, so it can never collide with a user-defined type. It is created
    /// once and reused, so every unknown type is [`Type::identical`] to every
    /// other, which keeps a single unresolved use from cascading into spurious
    /// type mismatches downstream; a diagnostic that follows the one reporting
    /// the unresolved use may still name it. The `span`, the span of the first
    /// unresolved use, seeds the synthetic definition node the first time the
    /// type is created. A follow-on diagnostic can show that span as the
    /// location where the type is defined: the "type is defined here" note of
    /// a type that is not displayable points at the first unresolved use,
    /// which may be an unrelated type name elsewhere in the model.
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
    pub fn get_qualified_name_from_map<S: SymbolInterface + Eq + std::hash::Hash>(
        parent_symbol_map: &HashMap<S, S>,
        symbol: &S,
    ) -> QualifiedName {
        let mut parts = vec![symbol.name().data.clone()];
        let mut current = symbol;
        while let Some(parent) = parent_symbol_map.get(current) {
            parts.push(parent.name().data.clone());
            current = parent;
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

    /// Compute the common type for a list of node Ids
    pub(crate) fn common_type(
        &self,
        nodes: &[fpp_core::Node],
        empty_list_error: SemanticError,
    ) -> SemanticResult<Arc<Type>> {
        let Some((first_node_id, rest)) = nodes.split_first() else {
            return Err(empty_list_error);
        };
        let mut prev_node_id = *first_node_id;
        let mut prev_type = self.type_map[first_node_id].clone();
        for node_id in rest {
            let current_type = &self.type_map[node_id];
            let loc = prev_node_id.span();
            prev_type = Analysis::common_type_of_types(&prev_type, current_type, loc)?;
            prev_node_id = *node_id;
        }
        Ok(prev_type)
    }

    /// Compute the common type for two node Ids
    pub(crate) fn common_type_of_pair(
        &self,
        id1: fpp_core::Node,
        id2: fpp_core::Node,
        error_loc: Span,
    ) -> SemanticResult<Arc<Type>> {
        let t1 = &self.type_map[&id1];
        let t2 = &self.type_map[&id2];
        Analysis::common_type_of_types(t1, t2, error_loc)
    }

    /// Add two nodes
    ///
    /// Integer values are `i128`, so an integer result that does not fit is an
    /// error at `loc`, the location of the operation.
    pub(crate) fn add(
        &self,
        id1: fpp_core::Node,
        id2: fpp_core::Node,
        loc: Span,
    ) -> SemanticResult<Value> {
        let v1 = &self.value_map[&id1];
        let v2 = &self.value_map[&id2];
        Analysis::arithmetic_result(v1.add(v2), loc, "addition failed")
    }

    /// Subtract one node from another
    pub(crate) fn sub(
        &self,
        id1: fpp_core::Node,
        id2: fpp_core::Node,
        loc: Span,
    ) -> SemanticResult<Value> {
        let v1 = &self.value_map[&id1];
        let v2 = &self.value_map[&id2];
        Analysis::arithmetic_result(v1.sub(v2), loc, "subtraction failed")
    }

    /// Multiply two nodes
    pub(crate) fn mul(
        &self,
        id1: fpp_core::Node,
        id2: fpp_core::Node,
        loc: Span,
    ) -> SemanticResult<Value> {
        let v1 = &self.value_map[&id1];
        let v2 = &self.value_map[&id2];
        Analysis::arithmetic_result(v1.mul(v2), loc, "multiplication failed")
    }

    /// Divide one node by another
    pub(crate) fn div(
        &self,
        id1: fpp_core::Node,
        id2: fpp_core::Node,
        loc: Span,
    ) -> SemanticResult<Value> {
        let v1 = &self.value_map[&id1];
        let v2 = &self.value_map[&id2];
        if v2.is_zero() {
            let loc = id2.span();
            Err(SemanticError::DivisionByZero { loc })
        } else {
            Analysis::arithmetic_result(v1.div(v2), loc, "division failed")
        }
    }

    /// Left shift an integer value
    pub(crate) fn lshift(
        &self,
        id1: fpp_core::Node,
        id2: fpp_core::Node,
        loc: Span,
    ) -> SemanticResult<Value> {
        self.shift(id1, id2, loc, Value::shl, "left shift")
    }

    /// Right shift an integer value
    pub(crate) fn rshift(
        &self,
        id1: fpp_core::Node,
        id2: fpp_core::Node,
        loc: Span,
    ) -> SemanticResult<Value> {
        self.shift(id1, id2, loc, Value::shr, "right shift")
    }

    /// Helper method to shift v1 by v2, checking that v2 is a valid shift amount
    ///
    /// A shift operand of an integer type may hold a floating-point value:
    /// negating a value of an alias of a floating-point type has type Integer.
    /// Such an operand is an error, as an operand of a floating-point type is.
    fn shift(
        &self,
        id1: fpp_core::Node,
        id2: fpp_core::Node,
        loc: Span,
        op: fn(&Value, &Value) -> MathResult,
        op_name: &str,
    ) -> SemanticResult<Value> {
        let v1 = &self.value_map[&id1];
        let v2 = &self.value_map[&id2];
        let not_an_integer = |id: fpp_core::Node, v: &Value| SemanticError::InvalidType {
            loc: id.span(),
            msg: format!("{} is not an integer or enum type", v.get_type()),
            notes: vec![],
        };

        let Some(shift_amount) = v2.as_shift_int() else {
            return Err(not_an_integer(id2, v2));
        };

        if !(0..=255).contains(&shift_amount) {
            let loc = id2.span();
            Err(SemanticError::InvalidShiftAmount { loc })
        } else {
            match op(v1, v2) {
                Ok(v) => Ok(v),
                Err(MathError::InvalidInputs) => Err(not_an_integer(id1, v1)),
                Err(MathError::ShiftOverflow) => Err(SemanticError::InvalidIntValue {
                    loc,
                    v: None,
                    msg: "shift result is too large to represent".to_string(),
                }),
                Err(_) => panic!("internal error: {op_name} failed"),
            }
        }
    }

    /// Negate a value
    ///
    /// `loc` is the location of the negation, where a result that does not fit
    /// in an `i128` is reported.
    pub(crate) fn neg(&self, id: fpp_core::Node, loc: Span) -> SemanticResult<Value> {
        let v = &self.value_map[&id];
        Analysis::arithmetic_result(v.negate(), loc, "negation failed")
    }

    /// The value computed by an arithmetic operation at `loc`. The operands of
    /// an operation that type checks are numbers, or strings for addition.
    fn arithmetic_result(result: MathResult, loc: Span, failure: &str) -> SemanticResult<Value> {
        match result {
            Ok(v) => Ok(v),
            Err(MathError::Overflow) => Err(SemanticError::InvalidIntValue {
                loc,
                v: None,
                msg: "arithmetic result is too large to represent".to_string(),
            }),
            Err(_) => panic!("internal error: {failure}"),
        }
    }

    /// Gets the finalized type of a type-name use node.
    ///
    /// `FinalizeTypeDefs` records the finalized type of a definition at the
    /// definition node and at each type name that it visits. A type name
    /// that it does not visit, such as the operand of a `sizeof`, keeps the
    /// type that `CheckTypeUses` gave it, which may not be finalized (an
    /// array whose size is not known, a struct with no default value).
    /// Resolving the use through its definition node gives the finalized type
    /// in either case.
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

    /// Gets a component instance from the component instance map
    pub fn get_component_instance(
        &self,
        id: fpp_core::Node,
    ) -> SemanticResult<Option<ComponentInstance>> {
        Ok(self
            .get_component_instance_symbol(id)?
            .and_then(|cis| self.component_instance_map.get(&cis).cloned()))
    }

    /// Gets an interface instance symbol from the use-def map
    pub fn get_interface_instance_symbol(
        &self,
        id: fpp_core::Node,
    ) -> SemanticResult<Option<Symbol>> {
        match self.use_def_map.get(&id) {
            Some(
                symbol @ (Symbol::ComponentInstance(_)
                | Symbol::Topology(_)
                | Symbol::TemplateInterfaceArg { .. }),
            ) => Ok(Some(symbol.clone())),
            Some(symbol) => Err(Analysis::invalid_symbol(
                symbol,
                id,
                "not a component instance or topology symbol",
            )),
            None => Ok(None),
        }
    }

    /// Gets an interface instance from the topology or component instance map
    pub fn get_interface_instance(
        &self,
        id: fpp_core::Node,
    ) -> SemanticResult<Option<InterfaceInstance>> {
        let Some(iis) = self.get_interface_instance_symbol(id)? else {
            return Ok(None);
        };
        match &iis {
            Symbol::ComponentInstance(_) => Ok(self
                .component_instance_map
                .get(&iis)
                .cloned()
                .map(InterfaceInstance::from_component_instance)),
            Symbol::Topology(_) => match self.topology_map.get(&iis) {
                Some(t) => Ok(Some(InterfaceInstance::from_topology(t))),
                None => Err(Analysis::invalid_symbol(
                    &iis,
                    id,
                    "this topology could not be resolved before it was used here",
                )),
            },
            Symbol::TemplateInterfaceArg { param_def, value } => {
                let Some(ii) = self.get_interface_instance(value.id())? else {
                    return Ok(None);
                };
                let Some(iface) = self.get_interface(param_def.interface.id())? else {
                    return Ok(None);
                };
                Ok(Some(InterfaceInstance::from_template_arg(
                    param_def.clone(),
                    iface,
                    ii,
                )))
            }
            _ => unreachable!("get_interface_instance_symbol returns an interface instance symbol"),
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

    /// Gets an interface from the interface definition map
    pub fn get_interface(&self, id: fpp_core::Node) -> SemanticResult<Option<Interface>> {
        Ok(self
            .get_interface_symbol(id)?
            .and_then(|symbol| self.interface_map.get(&symbol).cloned()))
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
            msg: msg.to_string(),
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

    /// Gets a BigInt value from an AST node
    pub fn get_big_int_value(&self, node: fpp_core::Node) -> Option<i128> {
        self.get_int_value(node)
    }

    /// Gets a BigInt value from an AST node
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

    /// Gets an optional BigInt value from an AST node
    pub fn get_big_int_value_opt(&self, expr: &Option<Expr>) -> Option<i128> {
        expr.as_ref().and_then(|e| self.get_int_value(e.node_id))
    }

    /// Gets an int value from an AST node
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

    /// Gets an array size from an AST node: an int value (in the i32 range)
    /// that is at least 1. A node with no value has size 1.
    pub fn get_array_size(
        &self,
        node: fpp_core::Node,
        loc: fpp_core::Span,
    ) -> SemanticResult<i128> {
        if self.get_int_value(node).is_none() {
            return Ok(1);
        }
        let v = self.get_int_value_checked(node, loc)?;
        if v >= 1 {
            Ok(v)
        } else {
            Err(SemanticError::InvalidArraySize { loc, size: v })
        }
    }

    /// Gets an optional array size
    ///
    /// The size is 1 when the expression is absent. The size of a present
    /// expression is checked as [`Analysis::get_array_size`] checks it.
    pub fn get_array_size_opt(&self, expr: &Option<Expr>) -> SemanticResult<i128> {
        match expr {
            Some(e) => self.get_array_size(e.node_id, e.span()),
            None => Ok(1),
        }
    }

    /// Gets a nonnegative BigInt value from an AST node
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

    /// Gets an optional nonnegative BigInt value from an AST node
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

    /// Gets a nonnegative int value from an AST node
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

    /// Gets an optional nonnegative int value from an AST node
    pub fn get_nonnegative_int_value_opt(
        &self,
        expr: &Option<Expr>,
    ) -> SemanticResult<Option<i128>> {
        match expr {
            Some(e) => Ok(Some(self.get_nonnegative_int_value(e.node_id, e.span())?)),
            None => Ok(None),
        }
    }

    /// Gets a queue full behavior from an AST node
    pub fn get_queue_full(opt: &Option<QueueFull>) -> QueueFull {
        opt.clone().unwrap_or(QueueFull::Assert)
    }

    /// Gets the number of ref params in a formal param list
    pub fn get_num_ref_params(params: &[FormalParam]) -> usize {
        params
            .iter()
            .filter(|p| matches!(p.kind, FormalParamKind::Ref))
            .count()
    }

    /// Compute a format from a format string and a list of types
    pub fn compute_format(
        node: &fpp_ast::LitString,
        ts: &[Arc<Type>],
    ) -> SemanticResult<crate::semantics::Format> {
        use crate::semantics::{Field, Parser};
        let loc = node.span();
        let format = Parser::parse_node(node)?;
        // Check the size
        if format.fields.len() < ts.len() {
            return Err(SemanticError::InvalidFormatString {
                loc,
                msg: "missing replacement field".to_string(),
                context: None,
            });
        }
        if format.fields.len() > ts.len() {
            return Err(SemanticError::InvalidFormatString {
                loc: format.fields[ts.len()].1,
                msg: "too many replacement fields".to_string(),
                context: None,
            });
        }
        // Check each numeric field against its type
        for (t, (field, field_loc, _)) in ts.iter().zip(format.fields.iter()) {
            let error = |msg: String| SemanticError::InvalidFormatString {
                loc: *field_loc,
                msg,
                context: None,
            };
            let u = Type::underlying_type(t);
            if field.is_integer() && !u.is_int() {
                return Err(error(format!("{t} is not an integer type")));
            }
            if let Field::Rational {
                precision: Some(precision),
                ..
            } = field
                && Field::exceeds_max_precision(precision)
            {
                return Err(error(format!(
                    "precision value {precision} is out of range"
                )));
            }
            if field.is_rational() && !u.is_float() {
                return Err(error(format!("{t} is not a floating-point type")));
            }
        }
        Ok(format.into())
    }

    /// Convert one type to another
    pub fn convert_types(loc: Span, from: &Arc<Type>, to: &Arc<Type>) -> SemanticResult<Arc<Type>> {
        match Type::convert(from, to) {
            Ok(()) => Ok(to.clone()),
            Err(reason) => Err(SemanticError::TypeMismatch {
                loc,
                msg: format!("cannot convert {} to {}", from, to),
                reason: match reason {
                    // The message already names the two types
                    TypeConversionError::Mismatch { .. } => None,
                    reason => Some(Box::new(reason)),
                },
            }),
        }
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
            // Element already there: report the error
            Some(prev) => Err(SemanticError::DuplicateIdValue {
                value: Analysis::display_id_value(id),
                loc: get_loc(&element),
                prev_loc: get_loc(prev),
            }),
            // New element: compute the new map and the new default ID
            None => {
                let next = id
                    .checked_add(1)
                    .ok_or_else(|| SemanticError::InvalidIntValue {
                        loc: get_loc(&element),
                        v: None,
                        msg: format!("the default identifier after {} is out of range", id),
                    })?;
                map.insert(id, element);
                Ok(next)
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
        for value in dictionary.values_in_order() {
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

    /// Compute the common type for two types
    pub(crate) fn common_type_of_types(
        t1: &Arc<Type>,
        t2: &Arc<Type>,
        error_loc: Span,
    ) -> SemanticResult<Arc<Type>> {
        match Type::common_type(t1, t2) {
            None => Err(SemanticError::TypeMismatch {
                loc: error_loc,
                msg: format!("cannot compute common type of {} and {}", t1, t2),
                reason: None,
            }),
            Some(t) => Ok(t),
        }
    }

    /// Check for duplicate node
    pub fn check_for_duplicate_node<T: Spanned>(
        get_name: impl Fn(&T) -> &str,
        error: impl Fn(String, Span, Span) -> SemanticError,
        nodes: &[T],
    ) -> SemanticResult {
        let mut map: HashMap<&str, Span> = HashMap::default();
        for node in nodes {
            let name = get_name(node);
            match map.get(name) {
                None => {
                    map.insert(name, node.span());
                }
                Some(prev_loc) => return Err(error(name.to_string(), node.span(), *prev_loc)),
            }
        }
        Ok(())
    }

    /// Check for duplicate struct member
    pub fn check_for_duplicate_struct_member<T: Spanned>(
        get_name: impl Fn(&T) -> &str,
        nodes: &[T],
    ) -> SemanticResult {
        Analysis::check_for_duplicate_node(
            get_name,
            |name, loc, prev_loc| SemanticError::DuplicateStructMember {
                name,
                loc,
                prev_loc,
            },
            nodes,
        )
    }

    /// Check for duplicate parameter
    pub fn check_for_duplicate_parameter(nodes: &[FormalParam]) -> SemanticResult {
        fn get_name(param: &FormalParam) -> &str {
            &param.name.data
        }
        Analysis::check_for_duplicate_node(
            get_name,
            |name, loc, prev_loc| SemanticError::DuplicateParameter {
                name,
                loc,
                prev_loc,
            },
            nodes,
        )
    }

    /// Checks whether a port instance is the specified general port
    pub fn is_general_port(
        &self,
        pi: &PortInstance,
        direction: Direction,
        port_type_name: &str,
    ) -> bool {
        match (pi.get_type(), pi.get_direction()) {
            (Some(PortInstanceType::DefPort(def)), Some(d)) => {
                self.get_qualified_name(&Symbol::Port(def)) == port_type_name && d == direction
            }
            _ => false,
        }
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

    /// Checks for a displayable type
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

    /// Checks that all parameters in a formal param list are displayable
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

    /// The scope of a definition, or the global scope
    pub fn get_symbol_scope(&self, symbol: &Option<Symbol>) -> &Scope {
        self.get_scope(&symbol.clone().into())
    }

    /// Gets the scope of a symbol for resolving names, or `None` if the symbol
    /// opens no scope. Includes symbols defined by a template expansion: inside
    /// an expansion, a module the expansion defines has the scope of the
    /// definitions the expansion introduced.
    pub fn lookup_scope(&self, symbol: &Symbol) -> Option<&Scope> {
        self.local_scope(symbol)
            .or_else(|| self.symbol_scope_map.get(symbol))
    }

    /// The scope open at one level of the nested scope stack
    pub fn get_scope(&self, owner: &ScopeOwner) -> &Scope {
        match owner {
            ScopeOwner::Global => &self.global_scope,
            ScopeOwner::Symbol(s) => self
                .lookup_scope(s)
                .unwrap_or_else(|| panic!("symbol {} does not have a scope", s.name().data)),
            ScopeOwner::TemplateParams(node) => &self.expansion_at(*node).param_scope,
            ScopeOwner::TemplateExpansion(node) => &self.expansion_at(*node).scope,
        }
    }

    /// The scope open at one level of the nested scope stack, for entering
    /// symbols into it
    pub fn get_scope_mut(&mut self, owner: &ScopeOwner) -> &mut Scope {
        match owner {
            ScopeOwner::Global => &mut self.global_scope,
            ScopeOwner::Symbol(s) => self
                .symbol_scope_map
                .get_mut(s)
                .unwrap_or_else(|| panic!("symbol {} does not have a scope", s.name().data)),
            ScopeOwner::TemplateParams(node) => &mut self.expansion_at_mut(*node).param_scope,
            ScopeOwner::TemplateExpansion(node) => &mut self.expansion_at_mut(*node).scope,
        }
    }

    /// Get a relative or absolute symbol from the nested scope. A relative name
    /// is looked up from the innermost scope outwards; an absolute name only in
    /// the outermost (global) scope.
    pub fn nested_scope_get(
        &self,
        is_absolute: bool,
        name_group: NameGroup,
        name: &str,
    ) -> Option<Symbol> {
        self.nested_scope.get(is_absolute, |owner| {
            self.get_scope(owner).get(name_group, name)
        })
    }

    /// Get a relative symbol from the nested scope
    pub fn symbol_get(&self, name_group: NameGroup, name: &str) -> Option<Symbol> {
        self.nested_scope_get(false, name_group, name)
    }

    /// Put a name and symbol into the innermost scope of the nested scope
    pub fn symbol_put(&mut self, name_group: NameGroup, symbol: Symbol) -> SemanticResult {
        let owner = self.nested_scope.current().clone();
        self.get_scope_mut(&owner).put(name_group, symbol)
    }

    /// The expansion of the template expansion specifier at `node`
    pub fn expansion_at(&self, node: fpp_core::Node) -> &TemplateExpansion {
        self.template_expansion_map
            .get(&node)
            .expect("template expansion specifier has no expansion")
    }

    fn expansion_at_mut(&mut self, node: fpp_core::Node) -> &mut TemplateExpansion {
        self.template_expansion_map
            .get_mut(&node)
            .expect("template expansion specifier has no expansion")
    }

    /// The scope of a module defined by the expansion we are analyzing inside of.
    ///
    /// A module defined by an expansion is merged with a same-named module at the
    /// expansion site, so the scope in the symbol scope map holds both sets of
    /// members. A use inside the expansion must see only the members that the
    /// expansion introduced: otherwise a definition at the expansion site
    /// captures a use of a bound parameter and the argument is discarded.
    fn local_scope(&self, symbol: &Symbol) -> Option<&Scope> {
        let expansion = self.expansion_at(self.template_expansion?);
        expansion.local_scope_map.get(symbol)
    }

    /// The nested scope in effect at the point of a definition, built from
    /// scratch: the global scope, with the scope of each enclosing definition
    /// pushed on top
    pub fn nested_scope_at(&self, symbol: &Symbol) -> NestedScope {
        let mut nested_scope = match self.parent_symbol_map.get(symbol) {
            None => self.nested_scope.global_scope(),
            Some(parent) => self.nested_scope_at(parent),
        };
        // A definition without a brace-delimited body opens no scope
        if self.symbol_scope_map.contains_key(symbol) {
            nested_scope.push(ScopeOwner::Symbol(symbol.clone()));
        }
        nested_scope
    }

    /// Gets a template symbol from the use-def map
    pub fn get_template_symbol(&self, id: fpp_core::Node) -> SemanticResult<Option<Symbol>> {
        match self.use_def_map.get(&id) {
            Some(symbol @ Symbol::Template(_)) => Ok(Some(symbol.clone())),
            Some(symbol) => Err(Analysis::invalid_symbol(
                symbol,
                id,
                "not a template symbol",
            )),
            None => Ok(None),
        }
    }

    /// Gets the use represented by a use. A use that refers to a bound template
    /// parameter represents the use appearing in the template argument, resolved
    /// recursively; any other use represents itself.
    pub(crate) fn get_represented_use(&self, id: fpp_core::Node) -> fpp_core::Node {
        match self.use_def_map.get(&id) {
            Some(symbol) if symbol.is_template_arg() => self.get_represented_use(symbol.node()),
            _ => id,
        }
    }

    /// Gets the symbol represented by a use, stripping off any bound template
    /// parameters. Returns `None` if the represented use refers to no symbol,
    /// for example if the template argument is a literal value or a primitive
    /// type name.
    pub fn get_represented_symbol(&self, id: fpp_core::Node) -> Option<&Symbol> {
        self.use_def_map.get(&self.get_represented_use(id))
    }

    /// Gets the topology symbol represented by a symbol, if there is one.
    /// See [`Analysis::get_represented_symbol`].
    pub fn get_represented_topology_symbol<'a>(&'a self, symbol: &'a Symbol) -> Option<&'a Symbol> {
        let represented = if symbol.is_template_arg() {
            self.get_represented_symbol(symbol.node())?
        } else {
            symbol
        };
        matches!(represented, Symbol::Topology(_)).then_some(represented)
    }

    /// Gets the component instance represented by a use. Strip off any template
    /// args.
    ///
    /// An error is reported at the use and names the symbol the use refers to,
    /// which for a bound parameter is the parameter; its note points at the
    /// represented definition. Returns `None` if the use, or the template
    /// argument it stands for, is unresolved, or if it names a component
    /// instance whose definition failed to check (both already reported).
    pub fn get_represented_component_instance(
        &self,
        id: fpp_core::Node,
    ) -> SemanticResult<Option<ComponentInstance>> {
        let Some(symbol) = self.use_def_map.get(&id) else {
            return Ok(None);
        };
        match self.get_represented_symbol(id) {
            Some(cis @ Symbol::ComponentInstance(_)) => {
                Ok(self.component_instance_map.get(cis).cloned())
            }
            None => Ok(None),
            Some(represented) => Err(SemanticError::InvalidSymbol {
                symbol_name: symbol.name().data.clone(),
                msg: "not a component instance symbol".to_string(),
                loc: id.span(),
                def_loc: represented.node().span(),
            }),
        }
    }
}

/// An ID-keyed dictionary map that [`Analysis::add_element_to_id_map`] can fill
pub trait IdMap<T> {
    fn get(&self, id: &i128) -> Option<&T>;
    fn insert(&mut self, id: i128, element: T);
    /// The values of the map in the order [`Analysis::check_dictionary_names`]
    /// visits them: insertion order for an insertion-ordered map, id order
    /// otherwise
    fn values_in_order(&self) -> Vec<&T>;
}

impl<T> IdMap<T> for HashMap<i128, T> {
    fn get(&self, id: &i128) -> Option<&T> {
        HashMap::get(self, id)
    }

    fn insert(&mut self, id: i128, element: T) {
        HashMap::insert(self, id, element);
    }

    fn values_in_order(&self) -> Vec<&T> {
        let mut ids: Vec<&i128> = self.keys().collect();
        ids.sort();
        ids.into_iter().map(|id| &self[id]).collect()
    }
}

impl<T> IdMap<T> for indexmap::IndexMap<i128, T, FxBuildHasher> {
    fn get(&self, id: &i128) -> Option<&T> {
        indexmap::IndexMap::get(self, id)
    }

    fn insert(&mut self, id: i128, element: T) {
        indexmap::IndexMap::insert(self, id, element);
    }

    fn values_in_order(&self) -> Vec<&T> {
        self.values().collect()
    }
}

impl<T> IdMap<T> for std::collections::BTreeMap<i128, T> {
    fn get(&self, id: &i128) -> Option<&T> {
        std::collections::BTreeMap::get(self, id)
    }

    fn insert(&mut self, id: i128, element: T) {
        std::collections::BTreeMap::insert(self, id, element);
    }

    fn values_in_order(&self) -> Vec<&T> {
        self.values().collect()
    }
}
