use crate::Analysis;
use fpp_core::Node;
use std::sync::Arc;

/// The interface for an FPP symbol
pub trait SymbolInterface: Clone {
    /// Gets the AST node ID of the symbol
    fn node(&self) -> Node;
    /// Gets the unqualified name of the symbol
    fn name(&self) -> &fpp_ast::Name;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DefModuleStub {
    pub node_id: Node,
    pub name: fpp_ast::Name,
}

impl From<&fpp_ast::DefModule> for DefModuleStub {
    fn from(value: &fpp_ast::DefModule) -> Self {
        Self {
            name: value.name.clone(),
            node_id: value.node_id,
        }
    }
}

/// A data structure that represents a definition
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Symbol {
    AbsType(Arc<fpp_ast::DefAbsType>),
    AliasType(Arc<fpp_ast::DefAliasType>),
    Array(Arc<fpp_ast::DefArray>),
    Component(Arc<fpp_ast::DefComponent>),
    ComponentInstance(Arc<fpp_ast::DefComponentInstance>),
    Constant(Arc<fpp_ast::DefConstant>),
    Enum(Arc<fpp_ast::DefEnum>),
    EnumConstant(Arc<fpp_ast::DefEnumConstant>),
    Interface(Arc<fpp_ast::DefInterface>),
    Module(Arc<DefModuleStub>),
    Port(Arc<fpp_ast::DefPort>),
    StateMachine(Arc<fpp_ast::DefStateMachine>),
    Struct(Arc<fpp_ast::DefStruct>),
    System(Arc<fpp_ast::DefSystem>),
    Topology(Arc<fpp_ast::DefTopology>),
}

impl Symbol {
    /// The unqualified (leaf) name of this symbol.
    pub fn unqualified_name(&self) -> &str {
        &self.name().data
    }

    /// The fully-qualified name of this symbol.
    pub fn qualified_name(&self, a: &Analysis) -> String {
        a.get_qualified_name(self)
    }

    /// The parent (enclosing) symbol, if any.
    pub fn parent<'a>(&self, a: &'a Analysis) -> Option<&'a Symbol> {
        a.parent_symbol_map.get(self)
    }

    /// Whether the definition named by this symbol is a dictionary definition.
    pub fn is_dictionary_def(&self) -> bool {
        match self {
            Symbol::AliasType(def) => def.is_dictionary_def,
            Symbol::Array(def) => def.is_dictionary_def,
            Symbol::Constant(def) => def.is_dictionary_def,
            Symbol::Enum(def) => def.is_dictionary_def,
            Symbol::Struct(def) => def.is_dictionary_def,
            _ => false,
        }
    }
}

impl SymbolInterface for Symbol {
    fn node(&self) -> Node {
        match self {
            Symbol::AbsType(node) => node.node_id,
            Symbol::AliasType(node) => node.node_id,
            Symbol::Array(node) => node.node_id,
            Symbol::Component(node) => node.node_id,
            Symbol::ComponentInstance(node) => node.node_id,
            Symbol::Constant(node) => node.node_id,
            Symbol::Enum(node) => node.node_id,
            Symbol::EnumConstant(node) => node.node_id,
            Symbol::Interface(node) => node.node_id,
            Symbol::Module(node) => node.node_id,
            Symbol::StateMachine(node) => node.node_id,
            Symbol::Struct(node) => node.node_id,
            Symbol::System(node) => node.node_id,
            Symbol::Topology(node) => node.node_id,
            Symbol::Port(node) => node.node_id,
        }
    }

    fn name(&self) -> &fpp_ast::Name {
        match self {
            Symbol::AbsType(def) => &def.name,
            Symbol::AliasType(def) => &def.name,
            Symbol::Array(def) => &def.name,
            Symbol::Component(def) => &def.name,
            Symbol::ComponentInstance(def) => &def.name,
            Symbol::Constant(def) => &def.name,
            Symbol::Enum(def) => &def.name,
            Symbol::EnumConstant(def) => &def.name,
            Symbol::Interface(def) => &def.name,
            Symbol::Module(def) => &def.name,
            Symbol::StateMachine(def) => &def.name,
            Symbol::Struct(def) => &def.name,
            Symbol::System(def) => &def.name,
            Symbol::Topology(def) => &def.name,
            Symbol::Port(def) => &def.name,
        }
    }
}
