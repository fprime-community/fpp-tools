pub mod component;
pub mod node;
pub mod state_machine;
pub mod topology;
pub mod visit;

use std::fmt::Debug;

use fpp_core::Annotated;
use fpp_macros::{AstAnnotated, DirectWalkable, VisitorWalkable, ast};

pub use component::*;
pub use node::*;
pub use state_machine::*;
pub use topology::*;
pub use visit::*;

pub trait AstNode: fpp_core::Spanned + Sized {
    fn id(&self) -> fpp_core::Node;
}

/// Translation unit
#[derive(Debug, Clone, VisitorWalkable)]
pub struct TransUnit(pub Vec<ModuleMember>);

pub enum QualIdentKind {
    Component,
    ComponentInstance,
    Constant,
    Port,
    Topology,
    Interface,
    Type,
    StateMachine,
}

#[ast]
#[derive(Debug, Clone, VisitorWalkable)]
pub struct LitString {
    #[visitable(ignore)]
    pub data: String,
    #[visitable(ignore)]
    pub inner_span: fpp_core::Span,
}

/// Definition name
#[ast]
#[derive(Debug, Clone, VisitorWalkable)]
pub struct Name {
    #[visitable(ignore)]
    pub data: String,
}

/// Identifier
#[ast]
#[derive(Debug, Clone, VisitorWalkable)]
pub struct Ident {
    #[visitable(ignore)]
    pub data: String,
}

/// Float type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatKind {
    F32,
    F64,
}

/// Int type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, DirectWalkable)]
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
#[derive(Debug, Clone, VisitorWalkable)]
pub struct TypeName {
    pub kind: TypeNameKind,
}

/// A qualified identifier
#[ast]
#[derive(Debug, Clone, VisitorWalkable)]
pub struct Qualified {
    pub qualifier: Box<QualIdent>,
    pub name: Ident,
}

/// A possibly-qualified identifier
#[ast]
#[derive(Clone, VisitorWalkable)]
pub enum QualIdent {
    /// An unqualified identifier
    Unqualified(Ident),
    Qualified(Qualified),
}

/// Struct member
#[ast]
#[derive(Debug, Clone, VisitorWalkable)]
pub struct StructExprMember {
    pub name: Name,
    pub value: Expr,
}

#[derive(Debug, Clone, DirectWalkable)]
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
    Ident(String),
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
#[derive(Debug, Clone, VisitorWalkable)]
pub struct Expr {
    pub kind: ExprKind,
}

/// Formal parameter kind
#[derive(Debug, Clone)]
pub enum FormalParamKind {
    Ref,
    Value,
}

/// Formal parameter
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct FormalParam {
    #[visitable(ignore)]
    pub kind: FormalParamKind,
    pub name: Name,
    pub type_name: TypeName,
}

/// Formal parameter list
pub type FormalParamList = Vec<FormalParam>;

/// Binary operation
#[derive(Debug, Clone)]
pub enum Binop {
    Add,
    Div,
    Mul,
    Sub,
    LShift,
    RShift,
}

/// Unary operation
#[derive(Debug, Clone)]
pub enum Unop {
    Minus,
}

/// Abstract type definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefAbsType {
    pub name: Name,
}

/// Aliased type definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefAliasType {
    pub name: Name,
    pub type_name: TypeName,
    #[visitable(ignore)]
    pub is_dictionary_def: bool,
}

/// Array definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
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
#[derive(Debug, Clone)]
pub enum ComponentKind {
    Active,
    Passive,
    Queued,
}

/// Component definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefComponent {
    #[visitable(ignore)]
    pub kind: ComponentKind,
    pub name: Name,
    pub members: Vec<ComponentMember>,
}

/// Component instance definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefComponentInstance {
    pub name: Name,
    pub component: QualIdent,
    pub base_id: Option<Expr>,
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
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct SpecInit {
    pub phase: Expr,
    #[visitable(ignore)]
    pub code: LitString,
}

/// Constant definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefConstant {
    pub name: Name,
    pub value: Expr,
    #[visitable(ignore)]
    pub is_dictionary_def: bool,
}

/// Enum definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
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
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefEnumConstant {
    pub name: Name,
    pub value: Option<Expr>,
}

/// Module definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefModule {
    pub name: Name,
    pub members: Vec<ModuleMember>,
}

/// Module member
#[ast]
#[derive(AstAnnotated, Clone, DirectWalkable)]
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
    DefPort(DefPort),
    DefStateMachine(DefStateMachine),
    DefStruct(DefStruct),
    DefSystem(DefSystem),
    DefTopology(DefTopology),
    SpecInclude(SpecInclude),
    SpecLoc(SpecLoc),
}

/// Location specifier kind
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpecLocKind {
    Component,
    Instance,
    Constant,
    Port,
    StateMachine,
    System,
    Type,
    Interface,
}

/// Location specifier
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
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
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct SpecGeneralPortInstance {
    #[visitable(ignore)]
    pub kind: GeneralPortInstanceKind,
    pub name: Name,
    pub size: Option<Expr>,
    pub port: Option<QualIdent>,
    pub priority: Option<Expr>,
    #[visitable(ignore)]
    pub queue_full: Option<QueueFull>,
}

/// Special port instance
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct SpecSpecialPortInstance {
    #[visitable(ignore)]
    pub input_kind: Option<InputPortKind>,
    #[visitable(ignore)]
    pub kind: SpecialPortInstanceKind,
    pub name: Name,
    pub priority: Option<Expr>,
    #[visitable(ignore)]
    pub queue_full: Option<QueueFull>,
}

/// Port instance specifier
#[ast]
#[derive(AstAnnotated, Clone, DirectWalkable)]
pub enum SpecPortInstance {
    General(SpecGeneralPortInstance),
    Special(SpecSpecialPortInstance),
}

/// Interface member
#[ast]
#[derive(AstAnnotated, Clone, DirectWalkable)]
pub enum InterfaceMember {
    SpecPortInstance(SpecPortInstance),
    SpecInterfaceImport(SpecInterfaceImport),
}

/// Interface definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefInterface {
    pub name: Name,
    pub members: Vec<InterfaceMember>,
}

/// Struct type member
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct StructTypeMember {
    pub name: Name,
    pub size: Option<Expr>,
    pub type_name: TypeName,
    #[visitable(ignore)]
    pub format: Option<LitString>,
}

/// Struct definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefStruct {
    pub name: Name,
    pub members: Vec<StructTypeMember>,
    pub default: Option<Expr>,
    #[visitable(ignore)]
    pub is_dictionary_def: bool,
}

/// Port definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefPort {
    pub name: Name,
    pub params: FormalParamList,
    pub return_type: Option<TypeName>,
}

/// Include specifier
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct SpecInclude {
    #[visitable(ignore)]
    pub file: LitString,
}

/// Import specifier
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct SpecInterfaceImport {
    pub interface: QualIdent,
}
