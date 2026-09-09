use crate::analysis::Analysis;
use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{NameGroup, Scope, Symbol, SymbolInterface};
use fpp_ast::*;
use fpp_core::Spanned;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Enter symbols into their scopes
pub struct EnterSymbols;

impl EnterSymbols {
    fn update_parent_symbol_map(&self, a: &mut Analysis, sym: Symbol) {
        match &a.parent_symbol {
            None => {}
            Some(parent) => {
                a.parent_symbol_map.insert(sym, parent.clone());
            }
        }
    }

    /// Enter a symbol into its own name group
    fn enter_symbol(&self, a: &mut Analysis, sym: Symbol, ng: NameGroup) -> SemanticResult {
        let res = a.symbol_put(ng, sym.clone());
        if res.is_ok() {
            // We successfully added the symbol to the scope
            // Update the parent symbol map
            self.update_parent_symbol_map(a, sym.clone());
        }

        res
    }
}

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
        let symbol = Symbol::Array(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::Type)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
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
        a.nested_scope.push(symbol.clone());

        let save_paren = a.parent_symbol.clone();
        a.parent_symbol = Some(symbol);
        let res = def.walk(a, self);
        a.parent_symbol = save_paren;
        a.nested_scope.pop();

        res
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
        let symbol = Symbol::Enum(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());

        (|| -> SemanticResult {
            self.enter_symbol(a, symbol.clone(), NameGroup::Type)?;
            self.enter_symbol(a, symbol.clone(), NameGroup::Value)?;
            Ok(())
        })()
        .unwrap_or_else(|err| err.emit());

        a.symbol_scope_map.insert(symbol.clone(), Scope::new());
        a.nested_scope.push(symbol.clone());

        let save_paren = a.parent_symbol.clone();
        a.parent_symbol = Some(symbol);
        let res = def.walk(a, self);
        a.parent_symbol = save_paren;
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

    fn visit_def_module(&self, a: &mut Analysis, def: &'ast DefModule) -> ControlFlow<Self::Break> {
        // Modules exist in all name groups and overlaps should not be detected as an error
        // We first need to check if a scope that this module will create already exists
        // If so we can just open that scope without adding a new one
        let existing_symbol = a
            .get_scope(a.nested_scope.current())
            .get(NameGroup::Value, &def.name.data);

        let sym = match existing_symbol {
            Some(other @ Symbol::Module(_)) => {
                // We found a module symbol with the same name at the current level.
                // Re-open the scope.
                a.symbol_map.insert(def.node_id, other.clone());
                other.clone()
            }
            Some(other) => {
                // We found a non-module symbol with the same name at the current level.
                // This is an error.
                SemanticError::RedefinedSymbol {
                    name: def.name.data.clone(),
                    loc: def.name.span(),
                    prev_loc: other.name().span(),
                }
                .emit();
                a.symbol_map.insert(def.node_id, other.clone());

                return ControlFlow::Continue(());
            }
            None => {
                // We did not find a symbol with the same name at the current level.
                // Create a new module symbol now.
                let sym = Symbol::Module(Arc::new(def.into()));
                a.symbol_map.insert(def.node_id, sym.clone());
                a.symbol_scope_map.insert(sym.clone(), Scope::new());

                for ng in NameGroup::all() {
                    match a.symbol_put(ng, sym.clone()) {
                        Ok(_) => {}
                        Err(err) => err.emit(),
                    }
                }

                sym
            }
        };

        self.update_parent_symbol_map(a, sym.clone());

        a.nested_scope.push(sym.clone());
        let save_paren = a.parent_symbol.clone();
        a.parent_symbol = Some(sym);
        let res = def.walk(a, self);
        a.parent_symbol = save_paren;

        a.nested_scope.pop();

        res
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
        a.nested_scope.push(symbol.clone());

        let save_parent = a.parent_symbol.clone();
        a.parent_symbol = Some(symbol);
        let res = def.walk(a, self);
        a.parent_symbol = save_parent;
        a.nested_scope.pop();

        res
    }

    fn visit_def_struct(&self, a: &mut Analysis, def: &'ast DefStruct) -> ControlFlow<Self::Break> {
        let symbol = Symbol::Struct(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::Type)
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

    fn visit_def_system(&self, a: &mut Analysis, def: &'ast DefSystem) -> ControlFlow<Self::Break> {
        let symbol = Symbol::System(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::System)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
    }

    fn visit_def_port(&self, a: &mut Self::State, def: &'ast DefPort) -> ControlFlow<Self::Break> {
        let symbol = Symbol::Port(Arc::new(def.clone()));
        a.symbol_map.insert(def.node_id, symbol.clone());
        self.enter_symbol(a, symbol, NameGroup::Port)
            .unwrap_or_else(|err| err.emit());
        ControlFlow::Continue(())
    }
}
