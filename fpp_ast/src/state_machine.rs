use crate::*;

/// State machine definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefStateMachine {
    pub name: Name,
    pub members: Option<Vec<StateMachineMember>>,
}

/// State machine member
#[ast]
#[derive(AstAnnotated, Clone, DirectWalkable)]
pub enum StateMachineMember {
    DefAbsType(DefAbsType),
    DefAliasType(DefAliasType),
    DefArray(DefArray),
    DefAction(DefAction),
    DefChoice(DefChoice),
    DefConstant(DefConstant),
    DefEnum(DefEnum),
    DefGuard(DefGuard),
    DefSignal(DefSignal),
    DefState(DefState),
    DefStruct(DefStruct),
    SpecInclude(SpecInclude),
    SpecInitialTransition(SpecInitialTransition),
}

/// Action definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefAction {
    pub name: Name,
    pub type_name: Option<TypeName>,
}

/// Choice definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefChoice {
    pub name: Name,
    pub guard: Ident,
    pub if_transition: TransitionExpr,
    pub else_transition: TransitionExpr,
}

/// Guard definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefGuard {
    pub name: Name,
    pub type_name: Option<TypeName>,
}

/// Transition expression
#[ast]
#[derive(Debug, Clone, VisitorWalkable)]
pub struct TransitionExpr {
    pub actions: Option<DoExpr>,
    pub target: QualIdent,
}

/// Signal definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefSignal {
    pub name: Name,
    pub type_name: Option<TypeName>,
}

/// State definition
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct DefState {
    pub name: Name,
    pub members: Vec<StateMember>,
}

/// State member
#[ast]
#[derive(AstAnnotated, Clone, DirectWalkable)]
pub enum StateMember {
    DefChoice(DefChoice),
    DefState(DefState),
    SpecInclude(SpecInclude),
    SpecInitialTransition(SpecInitialTransition),
    SpecStateEntry(SpecStateEntry),
    SpecStateExit(SpecStateExit),
    SpecStateTransition(SpecStateTransition),
}

/// Initial state specifier
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct SpecInitialTransition {
    pub transition: TransitionExpr,
}

/// State entry specifier
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct SpecStateEntry {
    pub actions: DoExpr,
}

/// State exit specifier
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct SpecStateExit {
    pub actions: DoExpr,
}

/// Transition specifier
#[ast]
#[derive(AstAnnotated, Clone, VisitorWalkable)]
pub struct SpecStateTransition {
    pub signal: Ident,
    pub guard: Option<Ident>,
    pub transition_or_do: TransitionOrDo,
}

#[ast]
#[derive(Debug, Clone, VisitorWalkable)]
pub struct DoExpr {
    pub actions: Vec<Ident>,
}

/// Transition or do within transition specifier
#[derive(Debug, Clone, DirectWalkable)]
pub enum TransitionOrDo {
    Transition(TransitionExpr),
    Do(DoExpr),
}
