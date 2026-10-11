pub mod component;
pub mod node;
pub mod replicate;
mod serde_annotate;
pub mod state_machine;
pub mod template;
pub mod topology;
pub mod visit;

use std::fmt::Debug;

use fpp_core::{Annotated, Spanned};
use fpp_macros::{AstAnnotated, DirectWalkable, Replicate, VisitorWalkable, ast};

pub use component::*;
pub use node::*;
pub use replicate::*;
pub use state_machine::*;
pub use template::*;
pub use topology::*;
pub use visit::*;

pub trait AstNode: fpp_core::Spanned + Sized {
    fn id(&self) -> fpp_core::Node;
}

/// Translation unit
#[derive(Debug, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct TransUnit(pub Vec<ModuleMember>);

#[ast]
#[derive(Debug, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct LitString {
    #[visitable(ignore)]
    pub data: String,
    #[visitable(ignore)]
    #[serde(skip)]
    pub inner_span: fpp_core::Span,
}

/// Definition name
#[ast]
#[derive(Debug, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct Name {
    #[visitable(ignore)]
    pub data: String,
}

/// Identifier
#[ast]
#[derive(Debug, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct Ident {
    #[visitable(ignore)]
    pub data: String,
}

/// Float type
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum FloatKind {
    F32,
    F64,
}

/// Writes the type name
impl std::fmt::Display for FloatKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            FloatKind::F32 => "F32",
            FloatKind::F64 => "F64",
        })
    }
}

/// Int type
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum IntegerKind {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
}

/// Writes the type name
impl std::fmt::Display for IntegerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            IntegerKind::U8 => "U8",
            IntegerKind::I8 => "I8",
            IntegerKind::U16 => "U16",
            IntegerKind::I16 => "I16",
            IntegerKind::U32 => "U32",
            IntegerKind::I32 => "I32",
            IntegerKind::U64 => "U64",
            IntegerKind::I64 => "I64",
        })
    }
}

#[derive(Debug, Clone, Replicate, DirectWalkable, serde::Serialize)]
pub enum TypeNameKind {
    #[visitable(ignore)]
    Bool,
    #[visitable(ignore)]
    Floating(FloatKind),
    #[visitable(ignore)]
    Integer(IntegerKind),
    QualIdent(QualIdent),
    String(Option<Expr>),
}

/// Type name
#[ast]
#[derive(Debug, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct TypeName {
    pub kind: TypeNameKind,
}

impl TypeName {
    /// Gets isAbsolute if the type name represents a qualified identifier
    pub fn get_is_absolute_opt(&self) -> Option<bool> {
        match &self.kind {
            TypeNameKind::QualIdent(name) => Some(name.is_absolute()),
            _ => None,
        }
    }
}

/// An unqualified identifier
///
/// A visitor walks straight through it to its name.
#[ast]
#[derive(Debug, Clone, Replicate, DirectWalkable, serde::Serialize)]
pub struct Unqualified {
    pub name: Ident,
    #[visitable(ignore)]
    pub is_absolute: bool,
}

impl Unqualified {
    /// Convert a qualified identifier to a list of identifiers
    pub fn to_ident_list(&self) -> Vec<String> {
        vec![self.name.data.clone()]
    }
}

/// A qualified identifier
#[ast]
#[derive(Debug, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct Qualified {
    pub qualifier: Box<QualIdent>,
    pub name: Ident,
}

impl Qualified {
    /// Whether the identifier is resolved absolutely
    pub fn is_absolute(&self) -> bool {
        self.qualifier.is_absolute()
    }

    /// Convert a qualified identifier to a list of identifiers
    pub fn to_ident_list(&self) -> Vec<String> {
        let mut out = self.qualifier.to_ident_list();
        out.push(self.name.data.clone());
        out
    }
}

/// A possibly-qualified identifier
#[ast]
#[derive(Clone, Replicate, VisitorWalkable)]
pub enum QualIdent {
    /// An unqualified identifier
    Unqualified(Unqualified),
    /// A qualified identifier
    Qualified(Qualified),
}

impl QualIdent {
    /// Whether the identifier is resolved absolutely
    pub fn is_absolute(&self) -> bool {
        match self {
            QualIdent::Unqualified(unqualified) => unqualified.is_absolute,
            QualIdent::Qualified(qualified) => qualified.is_absolute(),
        }
    }

    /// Convert a qualified identifier to a list of identifiers
    pub fn to_ident_list(&self) -> Vec<String> {
        match self {
            QualIdent::Unqualified(unqualified) => unqualified.to_ident_list(),
            QualIdent::Qualified(qualified) => qualified.to_ident_list(),
        }
    }

    /// Construct a qualified identifier from a node list
    ///
    /// Every level of a qualified identifier is a node of its own, so this is
    /// also the constructor of a qualified identifier node from a node list. The
    /// node of each level spans the identifiers of that level: `A.B` in `A.B.C`
    /// covers `A.B` only, and the outermost level covers the whole list. A leading
    /// `.` that makes the identifier absolute is not part of any of these spans.
    ///
    /// Panics if the node list is empty.
    pub fn from_node_list(node_list: QualIdentNodeList, is_absolute: bool) -> QualIdent {
        let head = node_list
            .first()
            .expect("node list should not be empty")
            .span();
        match qual_ident_node_list::split(node_list) {
            (qualifier, name) if qualifier.is_empty() => QualIdent::Unqualified(Unqualified {
                node_id: fpp_core::Node::new(name.span()),
                name,
                is_absolute,
            }),
            (qualifier, name) => {
                let qualifier = QualIdent::from_node_list(qualifier, is_absolute);
                QualIdent::Qualified(Qualified {
                    node_id: fpp_core::Node::new(span_through(head, name.span())),
                    qualifier: Box::new(qualifier),
                    name,
                })
            }
        }
    }
}

/// The span from the start of `first` to the end of `last`
fn span_through(first: fpp_core::Span, last: fpp_core::Span) -> fpp_core::Span {
    let start = first.start().pos();
    fpp_core::Span::new(
        first.file(),
        start,
        last.end().pos() - start,
        first.including_span(),
    )
}

/// A qualified identifier represented as a list of identifier nodes
/// This is useful during parsing
pub type QualIdentNodeList = Vec<Ident>;

/// Operations on a [`QualIdentNodeList`]
pub mod qual_ident_node_list {
    use crate::{Ident, QualIdentNodeList};

    /// Split a qualified identifier list into qualifier and name
    ///
    /// Panics if the node list is empty.
    pub fn split(mut node_list: QualIdentNodeList) -> (QualIdentNodeList, Ident) {
        let name = node_list.pop().expect("node list should not be empty");
        (node_list, name)
    }

    /// Get the qualifier
    ///
    /// Panics if the node list is empty.
    pub fn qualifier(node_list: &[Ident]) -> &[Ident] {
        match node_list.split_last() {
            Some((_, qualifier)) => qualifier,
            None => panic!("node list should not be empty"),
        }
    }

    /// Get the unqualified name
    ///
    /// Panics if the node list is empty.
    pub fn name(node_list: &[Ident]) -> &Ident {
        node_list.last().expect("node list should not be empty")
    }
}

/// Writes the dotted name, with a leading `.` if the name is absolute
impl std::fmt::Display for QualIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_absolute() {
            f.write_str(".")?;
        }
        f.write_str(&self.to_ident_list().join("."))
    }
}

/// Serializes as the dotted name (`"A.B.C"`, or `".A.B.C"` if the name is
/// absolute), not as nested `Qualified` tags.
impl serde::Serialize for QualIdent {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Struct member
#[ast]
#[derive(Debug, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct StructExprMember {
    pub name: Name,
    pub value: Expr,
}

#[derive(Debug, Clone, Replicate, DirectWalkable, serde::Serialize)]
pub enum ExprKind {
    Array(Vec<Expr>),
    ArraySubscript {
        e1: Box<Expr>,
        e2: Box<Expr>,
    },
    Binop {
        left: Box<Expr>,
        #[visitable(ignore)]
        op: Binop,
        right: Box<Expr>,
    },
    Dot {
        e: Box<Expr>,
        id: Ident,
    },
    #[visitable(ignore)]
    Ident {
        value: String,
        is_absolute: bool,
    },
    #[visitable(ignore)]
    LiteralBool(bool),
    #[visitable(ignore)]
    LiteralInt(String),
    #[visitable(ignore)]
    LiteralFloat(String),
    #[visitable(ignore)]
    LiteralString(String),
    Paren(Box<Expr>),
    SizeOf(Box<TypeName>),
    Struct(Vec<StructExprMember>),
    Unop {
        #[visitable(ignore)]
        op: Unop,
        e: Box<Expr>,
    },
}

/// Expression
#[ast]
#[derive(Debug, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct Expr {
    pub kind: ExprKind,
}

impl Expr {
    /// Gets the identifier list, if the expression represents a qualified identifier
    pub fn get_ident_list_opt(&self) -> Option<Vec<String>> {
        match &self.kind {
            ExprKind::Dot { e, id } => e.get_ident_list_opt().map(|mut il| {
                il.push(id.data.clone());
                il
            }),
            ExprKind::Ident { value, .. } => Some(vec![value.clone()]),
            _ => None,
        }
    }

    /// Gets isAbsolute, if the expression represents a qualified identifier
    pub fn get_is_absolute_opt(&self) -> Option<bool> {
        match &self.kind {
            ExprKind::Dot { e, .. } => e.get_is_absolute_opt(),
            ExprKind::Ident { is_absolute, .. } => Some(*is_absolute),
            _ => None,
        }
    }
}

/// Formal parameter kind
#[derive(Debug, Clone, serde::Serialize)]
pub enum FormalParamKind {
    Ref,
    Value,
}

/// Formal parameter
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct FormalParam {
    #[visitable(ignore)]
    pub kind: FormalParamKind,
    pub name: Name,
    pub type_name: TypeName,
}

/// Formal parameter list
pub type FormalParamList = Vec<FormalParam>;

/// Binary operation
#[derive(Debug, Clone, serde::Serialize)]
pub enum Binop {
    Add,
    Div,
    Mul,
    Sub,
    LShift,
    RShift,
}

/// Writes the operator
impl std::fmt::Display for Binop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Binop::Add => "+",
            Binop::Div => "/",
            Binop::Mul => "*",
            Binop::Sub => "-",
            Binop::LShift => "<<",
            Binop::RShift => ">>",
        })
    }
}

/// Unary operation
#[derive(Debug, Clone, serde::Serialize)]
pub enum Unop {
    Minus,
}

/// Writes the operator
impl std::fmt::Display for Unop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Unop::Minus => "-",
        })
    }
}

/// Abstract type definition
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct DefAbsType {
    pub name: Name,
}

/// Aliased type definition
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct DefAliasType {
    pub name: Name,
    pub type_name: TypeName,
    #[visitable(ignore)]
    pub is_dictionary_def: bool,
}

/// Array definition
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct DefArray {
    pub name: Name,
    pub size: Expr,
    pub elt_type: TypeName,
    pub default: Option<Expr>,
    #[visitable(ignore)]
    pub format: Option<LitString>,
    #[visitable(ignore)]
    pub is_dictionary_def: bool,
}

/// Component kind
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub enum ComponentKind {
    Active,
    Passive,
    Queued,
}

impl std::fmt::Display for ComponentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ComponentKind::Active => "active",
            ComponentKind::Passive => "passive",
            ComponentKind::Queued => "queued",
        };
        f.write_str(s)
    }
}

/// Component definition
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct DefComponent {
    #[visitable(ignore)]
    pub kind: ComponentKind,
    pub name: Name,
    pub members: Vec<ComponentMember>,
}

/// Component instance definition
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct DefComponentInstance {
    pub name: Name,
    pub component: QualIdent,
    pub base_id: Expr,
    #[visitable(ignore)]
    pub impl_type: Option<LitString>,
    #[visitable(ignore)]
    pub file: Option<LitString>,
    pub queue_size: Option<Expr>,
    pub stack_size: Option<Expr>,
    pub priority: Option<Expr>,
    pub cpu: Option<Expr>,
    pub init_specs: Vec<SpecInit>,
}

/// Init specifier
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct SpecInit {
    pub phase: Expr,
    #[visitable(ignore)]
    pub code: LitString,
}

/// Constant definition
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct DefConstant {
    pub name: Name,
    pub value: Expr,
    #[visitable(ignore)]
    pub is_dictionary_def: bool,
}

/// Enum definition
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct DefEnum {
    pub name: Name,
    pub type_name: Option<TypeName>,
    pub constants: Vec<DefEnumConstant>,
    pub default: Option<Expr>,
    #[visitable(ignore)]
    pub is_dictionary_def: bool,
}

/// Enum constant definition
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct DefEnumConstant {
    pub name: Name,
    pub value: Option<Expr>,
}

/// Module definition
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct DefModule {
    pub name: Name,
    pub members: Vec<ModuleMember>,
}

/// Module member
#[ast]
#[derive(AstAnnotated, Clone, Replicate, DirectWalkable, serde::Serialize)]
pub enum ModuleMember {
    DefAbsType(DefAbsType),
    DefAliasType(DefAliasType),
    DefArray(DefArray),
    DefComponent(DefComponent),
    DefComponentInstance(DefComponentInstance),
    DefConstant(DefConstant),
    DefEnum(DefEnum),
    DefInterface(DefInterface),
    DefModule(DefModule),
    DefModuleTemplate(DefModuleTemplate),
    DefPort(DefPort),
    DefStateMachine(DefStateMachine),
    DefStruct(DefStruct),
    DefSystem(DefSystem),
    DefTopology(DefTopology),
    SpecInclude(SpecInclude),
    SpecLoc(SpecLoc),
    SpecTemplateExpand(SpecTemplateExpand),
}

/// Location specifier kind
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub enum SpecLocKind {
    Component,
    Instance,
    Constant,
    Port,
    StateMachine,
    Template,
    System,
    Type,
    Interface,
}

/// Writes the location kind as it is spelled in a location specifier
impl std::fmt::Display for SpecLocKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SpecLocKind::Component => "component",
            SpecLocKind::Instance => "instance",
            SpecLocKind::Constant => "constant",
            SpecLocKind::Port => "port",
            SpecLocKind::StateMachine => "state machine",
            SpecLocKind::Template => "template",
            SpecLocKind::System => "system",
            SpecLocKind::Type => "type",
            SpecLocKind::Interface => "interface",
        })
    }
}

/// Location specifier
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct SpecLoc {
    #[visitable(ignore)]
    pub kind: SpecLocKind,
    pub symbol: QualIdent,
    #[visitable(ignore)]
    pub file: LitString,
    #[visitable(ignore)]
    pub is_dictionary_def: bool,
}

/// General port instance
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct SpecGeneralPortInstance {
    #[visitable(ignore)]
    pub kind: GeneralPortInstanceKind,
    pub name: Name,
    pub size: Option<Expr>,
    pub port: Option<QualIdent>,
    pub priority: Option<Expr>,
    #[visitable(ignore)]
    pub queue_full: Option<QueueFullSpecifier>,
}

/// Special port instance
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct SpecSpecialPortInstance {
    #[visitable(ignore)]
    pub input_kind: Option<InputPortKind>,
    #[visitable(ignore)]
    pub kind: SpecialPortInstanceKind,
    pub name: Name,
    pub priority: Option<Expr>,
    #[visitable(ignore)]
    pub queue_full: Option<QueueFullSpecifier>,
}

/// Port instance specifier
#[ast]
#[derive(AstAnnotated, Clone, Replicate, DirectWalkable, serde::Serialize)]
pub enum SpecPortInstance {
    General(SpecGeneralPortInstance),
    Special(SpecSpecialPortInstance),
}

/// Interface member
#[ast]
#[derive(AstAnnotated, Clone, Replicate, DirectWalkable, serde::Serialize)]
pub enum InterfaceMember {
    SpecPortInstance(SpecPortInstance),
    SpecInterfaceImport(SpecInterfaceImport),
}

/// Interface definition
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct DefInterface {
    pub name: Name,
    pub members: Vec<InterfaceMember>,
}

/// Struct type member
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct StructTypeMember {
    pub name: Name,
    pub size: Option<Expr>,
    pub type_name: TypeName,
    #[visitable(ignore)]
    pub format: Option<LitString>,
}

/// Struct definition
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct DefStruct {
    pub name: Name,
    pub members: Vec<StructTypeMember>,
    pub default: Option<Expr>,
    #[visitable(ignore)]
    pub is_dictionary_def: bool,
}

/// Port definition
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct DefPort {
    pub name: Name,
    pub params: FormalParamList,
    pub return_type: Option<TypeName>,
}

/// Include specifier
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct SpecInclude {
    #[visitable(ignore)]
    pub file: LitString,
}

/// Import specifier
#[ast]
#[derive(AstAnnotated, Clone, Replicate, VisitorWalkable, serde::Serialize)]
pub struct SpecInterfaceImport {
    pub interface: QualIdent,
}

#[cfg(test)]
mod tests {
    use crate::*;

    /// Each enum is written as it is spelled in FPP source
    #[test]
    fn enum_spellings() {
        assert_eq!(FloatKind::F64.to_string(), "F64");
        assert_eq!(IntegerKind::U32.to_string(), "U32");
        assert_eq!(Binop::LShift.to_string(), "<<");
        assert_eq!(Unop::Minus.to_string(), "-");
        assert_eq!(SpecLocKind::StateMachine.to_string(), "state machine");
        assert_eq!(InputPortKind::Guarded.to_string(), "guarded");
        assert_eq!(QueueFull::Hook.to_string(), "hook");
        assert_eq!(EventSeverity::WarningHigh.to_string(), "warning high");
        assert_eq!(TlmChannelUpdate::OnChange.to_string(), "on change");
        assert_eq!(TlmChannelLimitKind::Orange.to_string(), "orange");
        assert_eq!(ConnectionPatternKind::TextEvent.to_string(), "text event");
        assert_eq!(
            GeneralPortInstanceKind::Input(InputPortKind::Async).to_string(),
            "async input"
        );
        assert_eq!(GeneralPortInstanceKind::Output.to_string(), "output");
    }
}
