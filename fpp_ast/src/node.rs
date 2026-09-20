use crate::*;
use fpp_core::Span;
use fpp_macros::DirectRefWalkable;

macro_rules! node_kinds {
    ($($ty:ident,)+) => {
        /// This enum is a super variant of all the types of nodes in the AST
        /// This allows implementing highly generic visitors that can just match
        /// recursively on any node in the AST.
        ///
        /// This allows for composing together "analyzers" which look at various
        /// parts of the AST and build.
        #[derive(Debug, Clone, Copy, DirectRefWalkable)]
        pub enum Node<'a> {
            $( $ty(&'a $ty), )+
        }

        impl<'a> Node<'a> {
            pub const KIND_NAMES: &'static [&'static str] = &[$( stringify!($ty), )+];

            pub fn kind_name(&self) -> &'static str {
                match self {
                    $( Node::$ty(_) => stringify!($ty), )+
                }
            }
        }

        /// Serializes as the referenced node itself, with no variant tag.
        impl serde::Serialize for Node<'_> {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                match self {
                    $( Node::$ty(node) => node.serialize(serializer), )+
                }
            }
        }
    };
}

node_kinds! {
    /* Definitions */
    DefAbsType,
    DefAction,
    DefAliasType,
    DefArray,
    DefChoice,
    DefComponent,
    DefComponentInstance,
    DefConstant,
    DefEnum,
    DefEnumConstant,
    DefGuard,
    DefInterface,
    DefModule,
    DefPort,
    DefSignal,
    DefState,
    DefStateMachine,
    DefStruct,
    DefSystem,
    DefTopology,
    /* Specifiers */
    SpecCommand,
    SpecDirectConnectionGraph,
    SpecPatternConnectionGraph,
    SpecContainer,
    SpecEvent,
    SpecGeneralPortInstance,
    SpecInterfaceImport,
    SpecInclude,
    SpecInit,
    SpecInitialTransition,
    SpecInstance,
    SpecInternalPort,
    SpecLoc,
    SpecParam,
    SpecPortInstance,
    SpecPortMatching,
    SpecRecord,
    SpecSpecialPortInstance,
    SpecStateEntry,
    SpecStateExit,
    SpecStateMachineInstance,
    SpecStateTransition,
    SpecTlmChannel,
    SpecTlmPacket,
    SpecTlmPacketSet,
    SpecTopPort,
    /* Other AST nodes */
    Expr,
    FormalParam,
    Name,
    Ident,
    LitString,
    QualIdent,
    Qualified,
    StructExprMember,
    TypeName,
    /* Inner AST nodes */
    Connection,
    DoExpr,
    EventThrottle,
    PortInstanceIdentifier,
    StructTypeMember,
    TlmChannelIdentifier,
    TlmChannelLimit,
    TransitionExpr,
}

impl<'a> Node<'a> {
    /// The unqualified name this node introduces, if any.
    pub fn name(&self) -> Option<&'a str> {
        match self {
            Node::DefAbsType(n) => Some(&n.name.data),
            Node::DefAction(n) => Some(&n.name.data),
            Node::DefAliasType(n) => Some(&n.name.data),
            Node::DefArray(n) => Some(&n.name.data),
            Node::DefChoice(n) => Some(&n.name.data),
            Node::DefComponent(n) => Some(&n.name.data),
            Node::DefComponentInstance(n) => Some(&n.name.data),
            Node::DefConstant(n) => Some(&n.name.data),
            Node::DefEnum(n) => Some(&n.name.data),
            Node::DefEnumConstant(n) => Some(&n.name.data),
            Node::DefGuard(n) => Some(&n.name.data),
            Node::DefInterface(n) => Some(&n.name.data),
            Node::DefModule(n) => Some(&n.name.data),
            Node::DefPort(n) => Some(&n.name.data),
            Node::DefSignal(n) => Some(&n.name.data),
            Node::DefState(n) => Some(&n.name.data),
            Node::DefStateMachine(n) => Some(&n.name.data),
            Node::DefStruct(n) => Some(&n.name.data),
            Node::DefSystem(n) => Some(&n.name.data),
            Node::DefTopology(n) => Some(&n.name.data),
            Node::SpecCommand(n) => Some(&n.name.data),
            Node::SpecContainer(n) => Some(&n.name.data),
            Node::SpecDirectConnectionGraph(n) => Some(&n.name.data),
            Node::SpecEvent(n) => Some(&n.name.data),
            Node::SpecGeneralPortInstance(n) => Some(&n.name.data),
            Node::SpecInternalPort(n) => Some(&n.name.data),
            Node::SpecParam(n) => Some(&n.name.data),
            Node::SpecRecord(n) => Some(&n.name.data),
            Node::SpecSpecialPortInstance(n) => Some(&n.name.data),
            Node::SpecStateMachineInstance(n) => Some(&n.name.data),
            Node::SpecTlmChannel(n) => Some(&n.name.data),
            Node::SpecTlmPacket(n) => Some(&n.name.data),
            Node::SpecTlmPacketSet(n) => Some(&n.name.data),
            Node::SpecTopPort(n) => Some(&n.name.data),
            Node::SpecPortInstance(n) => match n {
                SpecPortInstance::General(g) => Some(&g.name.data),
                SpecPortInstance::Special(s) => Some(&s.name.data),
            },
            Node::FormalParam(n) => Some(&n.name.data),
            Node::StructExprMember(n) => Some(&n.name.data),
            Node::StructTypeMember(n) => Some(&n.name.data),
            Node::Name(n) => Some(&n.data),
            Node::Ident(n) => Some(&n.data),
            Node::Connection(_)
            | Node::DoExpr(_)
            | Node::EventThrottle(_)
            | Node::Expr(_)
            | Node::LitString(_)
            | Node::PortInstanceIdentifier(_)
            | Node::QualIdent(_)
            | Node::Qualified(_)
            | Node::SpecInclude(_)
            | Node::SpecInit(_)
            | Node::SpecInitialTransition(_)
            | Node::SpecInstance(_)
            | Node::SpecInterfaceImport(_)
            | Node::SpecLoc(_)
            | Node::SpecPatternConnectionGraph(_)
            | Node::SpecPortMatching(_)
            | Node::SpecStateEntry(_)
            | Node::SpecStateExit(_)
            | Node::SpecStateTransition(_)
            | Node::TlmChannelIdentifier(_)
            | Node::TlmChannelLimit(_)
            | Node::TransitionExpr(_)
            | Node::TypeName(_) => None,
        }
    }
}
