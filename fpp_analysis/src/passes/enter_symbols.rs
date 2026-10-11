use crate::analysis::Analysis;
use crate::errors::SemanticResult;
use crate::passes::enter_module_symbols::EnterModuleSymbols;
use crate::semantics::{NameGroup, Scope, ScopeOwner, Symbol};
use fpp_ast::*;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Enter symbols into their scopes
pub struct EnterSymbols;

impl EnterSymbols {
    /// Enter a symbol into a name group of the current scope and, if that
    /// succeeds, record its parent symbol
    fn enter_symbol(&self, a: &mut Analysis, symbol: Symbol, ng: NameGroup) -> SemanticResult {
        let res = a.symbol_put(ng, symbol.clone());
        if res.is_ok() {
            self.update_map(a, symbol);
        }

        res
    }
}

impl<'ast> EnterModuleSymbols<'ast> for EnterSymbols {}

impl<'ast> Visitor<'ast> for EnterSymbols {
    type Break = ();
    type State = Analysis;

    fn visit_trans_unit(
        &self,
        a: &mut Self::State,
        node: &'ast TransUnit,
    ) -> ControlFlow<Self::Break> {
        node.walk(a, self)
    }

    fn visit_def_module(&self, a: &mut Analysis, def: &'ast DefModule) -> ControlFlow<Self::Break> {
        self.def_module(a, def)
    }

    fn visit_def_abs_type(
        &self,
        a: &mut Analysis,
        def: &'ast DefAbsType,
    ) -> ControlFlow<Self::Break> {
        let symbol = Symbol::AbsType(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::Type)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
    }

    fn visit_def_alias_type(
        &self,
        a: &mut Analysis,
        def: &'ast DefAliasType,
    ) -> ControlFlow<Self::Break> {
        let symbol = Symbol::AliasType(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::Type)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
    }

    fn visit_def_array(&self, a: &mut Analysis, def: &'ast DefArray) -> ControlFlow<Self::Break> {
        let symbol = Symbol::ArrayType(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::Type)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
    }

    fn visit_def_component(
        &self,
        a: &mut Analysis,
        def: &'ast DefComponent,
    ) -> ControlFlow<Self::Break> {
        let symbol = Symbol::Component(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());

        (|| -> SemanticResult {
            self.enter_symbol(a, symbol.clone(), NameGroup::Component)?;
            self.enter_symbol(a, symbol.clone(), NameGroup::StateMachine)?;
            self.enter_symbol(a, symbol.clone(), NameGroup::Type)?;
            self.enter_symbol(a, symbol.clone(), NameGroup::Value)?;

            Ok(())
        })()
        .unwrap_or_else(|err| err.emit());

        a.symbol_scope_map.insert(symbol.clone(), Scope::new());
        a.nested_scope.push(ScopeOwner::Symbol(symbol.clone()));

        let parent_symbol = a.parent_symbol.clone();
        a.parent_symbol = Some(symbol);
        let res = def.walk(a, self);
        a.parent_symbol = parent_symbol;
        a.nested_scope.pop();

        res
    }

    fn visit_def_component_instance(
        &self,
        a: &mut Analysis,
        def: &'ast DefComponentInstance,
    ) -> ControlFlow<Self::Break> {
        let symbol = Symbol::ComponentInstance(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::PortInterfaceInstance)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
    }

    fn visit_def_constant(
        &self,
        a: &mut Analysis,
        def: &'ast DefConstant,
    ) -> ControlFlow<Self::Break> {
        let symbol = Symbol::Constant(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::Value)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
    }

    fn visit_def_enum(&self, a: &mut Analysis, def: &'ast DefEnum) -> ControlFlow<Self::Break> {
        let symbol = Symbol::EnumType(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());

        (|| -> SemanticResult {
            self.enter_symbol(a, symbol.clone(), NameGroup::Type)?;
            self.enter_symbol(a, symbol.clone(), NameGroup::Value)?;
            Ok(())
        })()
        .unwrap_or_else(|err| err.emit());

        a.symbol_scope_map.insert(symbol.clone(), Scope::new());
        a.nested_scope.push(ScopeOwner::Symbol(symbol.clone()));

        let parent_symbol = a.parent_symbol.clone();
        a.parent_symbol = Some(symbol);
        let res = def.walk(a, self);
        a.parent_symbol = parent_symbol;
        a.nested_scope.pop();

        res
    }

    fn visit_def_enum_constant(
        &self,
        a: &mut Analysis,
        def: &'ast DefEnumConstant,
    ) -> ControlFlow<Self::Break> {
        let symbol = Symbol::EnumConstant(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::Value)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
    }

    fn visit_def_interface(
        &self,
        a: &mut Analysis,
        def: &'ast DefInterface,
    ) -> ControlFlow<Self::Break> {
        let symbol = Symbol::Interface(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::PortInterface)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
    }

    /// A module template definition introduces a name in the template name
    /// group. Its body is not entered here: the definitions in the body come
    /// into being only where the template is expanded.
    fn visit_def_module_template(
        &self,
        a: &mut Analysis,
        def: &'ast DefModuleTemplate,
    ) -> ControlFlow<Self::Break> {
        let symbol = Symbol::Template(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::Template)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
    }

    /// A template expansion specifier enters no symbols. The symbols of an
    /// expansion are entered by [`crate::passes::EnterTemplateExpansionSymbols`],
    /// after the template body has been copied to the point of the specifier.
    fn visit_spec_template_expand(
        &self,
        _a: &mut Analysis,
        _def: &'ast SpecTemplateExpand,
    ) -> ControlFlow<Self::Break> {
        ControlFlow::Continue(())
    }

    fn visit_def_port(&self, a: &mut Self::State, def: &'ast DefPort) -> ControlFlow<Self::Break> {
        let symbol = Symbol::Port(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::Port)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
    }

    fn visit_def_state_machine(
        &self,
        a: &mut Analysis,
        def: &'ast DefStateMachine,
    ) -> ControlFlow<Self::Break> {
        let symbol = Symbol::StateMachine(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());

        // A state machine is visible as a state machine, a type, and a value.
        (|| -> SemanticResult {
            self.enter_symbol(a, symbol.clone(), NameGroup::StateMachine)?;
            self.enter_symbol(a, symbol.clone(), NameGroup::Type)?;
            self.enter_symbol(a, symbol.clone(), NameGroup::Value)?;
            Ok(())
        })()
        .unwrap_or_else(|err| err.emit());

        // A state machine opens a scope for its nested definitions
        // (types, constants, etc.).
        a.symbol_scope_map.insert(symbol.clone(), Scope::new());
        a.nested_scope.push(ScopeOwner::Symbol(symbol.clone()));

        let parent_symbol = a.parent_symbol.clone();
        a.parent_symbol = Some(symbol);
        let res = def.walk(a, self);
        a.parent_symbol = parent_symbol;
        a.nested_scope.pop();

        res
    }

    fn visit_def_struct(&self, a: &mut Analysis, def: &'ast DefStruct) -> ControlFlow<Self::Break> {
        let symbol = Symbol::StructType(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::Type)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
    }

    fn visit_def_system(&self, a: &mut Analysis, def: &'ast DefSystem) -> ControlFlow<Self::Break> {
        let symbol = Symbol::System(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::System)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
    }

    fn visit_def_topology(
        &self,
        a: &mut Analysis,
        def: &'ast DefTopology,
    ) -> ControlFlow<Self::Break> {
        let symbol = Symbol::Topology(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::PortInterfaceInstance)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
    }
}
