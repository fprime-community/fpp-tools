use fpp_ast::AstNode;
use fpp_core::{Node, Span, Spanned};
use std::sync::Arc;

use crate::Analysis;

/// The interface for an FPP symbol
pub trait SymbolInterface: Clone {
    /// Gets the AST node ID of the symbol
    fn node(&self) -> Node;
    /// Gets the unqualified name of the symbol
    fn name(&self) -> &fpp_ast::Name;
    /// Gets the location of the symbol
    fn get_loc(&self) -> Span {
        self.node().span()
    }
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
    ArrayType(Arc<fpp_ast::DefArray>),
    Component(Arc<fpp_ast::DefComponent>),
    ComponentInstance(Arc<fpp_ast::DefComponentInstance>),
    Constant(Arc<fpp_ast::DefConstant>),
    EnumType(Arc<fpp_ast::DefEnum>),
    EnumConstant(Arc<fpp_ast::DefEnumConstant>),
    Interface(Arc<fpp_ast::DefInterface>),
    Module(Arc<DefModuleStub>),
    Port(Arc<fpp_ast::DefPort>),
    StateMachine(Arc<fpp_ast::DefStateMachine>),
    StructType(Arc<fpp_ast::DefStruct>),
    System(Arc<fpp_ast::DefSystem>),
    Template(Arc<fpp_ast::DefModuleTemplate>),
    Topology(Arc<fpp_ast::DefTopology>),
    /// A constant template parameter bound to a constant template argument
    TemplateConstantArg {
        param_def: Arc<fpp_ast::TemplateParamConstant>,
        value: Arc<fpp_ast::Expr>,
    },
    /// A type template parameter bound to a type template argument
    TemplateTypeArg {
        param_def: Arc<fpp_ast::TemplateParamType>,
        value: Arc<fpp_ast::TypeName>,
    },
    /// A port interface instance template parameter bound to a port interface
    /// instance template argument
    TemplateInterfaceArg {
        param_def: Arc<fpp_ast::TemplateParamInterface>,
        value: Arc<fpp_ast::QualIdent>,
    },
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
            Symbol::ArrayType(def) => def.is_dictionary_def,
            Symbol::Constant(def) => def.is_dictionary_def,
            Symbol::EnumType(def) => def.is_dictionary_def,
            Symbol::StructType(def) => def.is_dictionary_def,
            _ => false,
        }
    }

    /// Whether this symbol is a bound template parameter. A bound template
    /// parameter is not a global definition, so it has no qualified name.
    pub fn is_template_arg(&self) -> bool {
        matches!(
            self,
            Symbol::TemplateConstantArg { .. }
                | Symbol::TemplateTypeArg { .. }
                | Symbol::TemplateInterfaceArg { .. }
        )
    }

    /// Whether this symbol may be a port interface instance
    pub fn is_interface_instance(&self) -> bool {
        matches!(
            self,
            Symbol::ComponentInstance(_)
                | Symbol::Topology(_)
                | Symbol::TemplateInterfaceArg { .. }
        )
    }
}

impl SymbolInterface for Symbol {
    fn node(&self) -> Node {
        match self {
            Symbol::AbsType(node) => node.node_id,
            Symbol::AliasType(node) => node.node_id,
            Symbol::ArrayType(node) => node.node_id,
            Symbol::Component(node) => node.node_id,
            Symbol::ComponentInstance(node) => node.node_id,
            Symbol::Constant(node) => node.node_id,
            Symbol::EnumType(node) => node.node_id,
            Symbol::EnumConstant(node) => node.node_id,
            Symbol::Interface(node) => node.node_id,
            Symbol::Module(node) => node.node_id,
            Symbol::StateMachine(node) => node.node_id,
            Symbol::StructType(node) => node.node_id,
            Symbol::System(node) => node.node_id,
            Symbol::Template(node) => node.node_id,
            Symbol::Topology(node) => node.node_id,
            Symbol::Port(node) => node.node_id,
            // A bound template parameter is identified by the argument bound to
            // it, so that two expansions of the same template with different
            // arguments give distinct symbols
            Symbol::TemplateConstantArg { value, .. } => value.node_id,
            Symbol::TemplateTypeArg { value, .. } => value.node_id,
            Symbol::TemplateInterfaceArg { value, .. } => value.id(),
        }
    }

    fn name(&self) -> &fpp_ast::Name {
        match self {
            Symbol::AbsType(def) => &def.name,
            Symbol::AliasType(def) => &def.name,
            Symbol::ArrayType(def) => &def.name,
            Symbol::Component(def) => &def.name,
            Symbol::ComponentInstance(def) => &def.name,
            Symbol::Constant(def) => &def.name,
            Symbol::EnumType(def) => &def.name,
            Symbol::EnumConstant(def) => &def.name,
            Symbol::Interface(def) => &def.name,
            Symbol::Module(def) => &def.name,
            Symbol::StateMachine(def) => &def.name,
            Symbol::StructType(def) => &def.name,
            Symbol::System(def) => &def.name,
            Symbol::Template(def) => &def.name,
            Symbol::Topology(def) => &def.name,
            Symbol::Port(def) => &def.name,
            Symbol::TemplateConstantArg { param_def, .. } => &param_def.name,
            Symbol::TemplateTypeArg { param_def, .. } => &param_def.name,
            Symbol::TemplateInterfaceArg { param_def, .. } => &param_def.name,
        }
    }
}
