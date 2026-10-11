use crate::Analysis;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::component_analyzer::ComponentAnalyzer;
use crate::analyzers::interface_analyzer::InterfaceAnalyzer;
use crate::analyzers::module_analyzer::ModuleAnalyzer;
use crate::analyzers::state_machine_analyzer::StateMachineAnalyzer;
use crate::analyzers::template_expand_analyzer::TemplateExpandAnalyzer;
use crate::analyzers::tlm_packet_set_analyzer::TlmPacketSetAnalyzer;
use crate::analyzers::topology_analyzer::TopologyAnalyzer;
use fpp_ast::{Expr, ExprKind, Node, TemplateParam, TypeNameKind, Visitable, Visitor};
use std::marker::PhantomData;
use std::ops::ControlFlow;

/// The analyzers of the definitions and specifiers with members that the type
/// expression analysis builds on
type MemberAnalyzer = TemplateExpandAnalyzer<
    TopologyAnalyzer<
        TlmPacketSetAnalyzer<
            StateMachineAnalyzer<ModuleAnalyzer<InterfaceAnalyzer<ComponentAnalyzer>>>,
        >,
    >,
>;

/// Analyze types and expressions
///
/// The analyzer visits the types and the expressions of each definition and
/// specifier, and the members of the definitions and specifiers that have
/// members.
pub struct TypeExpressionAnalyzer<'ast, V: Visitor<'ast, State = Analysis>> {
    super_: MemberAnalyzer,
    phantom_data: PhantomData<&'ast V>,
}

impl<'ast, V: Visitor<'ast, State = Analysis>> Default for TypeExpressionAnalyzer<'ast, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast, V: Visitor<'ast, State = Analysis>> TypeExpressionAnalyzer<'ast, V> {
    pub fn new() -> TypeExpressionAnalyzer<'ast, V> {
        TypeExpressionAnalyzer {
            super_: MemberAnalyzer::default(),
            phantom_data: PhantomData,
        }
    }

    fn expr_node(&self, visitor: &V, a: &mut Analysis, node: &'ast Expr) -> ControlFlow<V::Break> {
        match &node.kind {
            ExprKind::Array(elts) => elts.visit(a, visitor),
            ExprKind::ArraySubscript { e1, e2 } => {
                e1.visit(a, visitor)?;
                e2.visit(a, visitor)
            }
            ExprKind::Binop { left, right, .. } => {
                left.visit(a, visitor)?;
                right.visit(a, visitor)
            }
            ExprKind::Dot { e, .. } => e.visit(a, visitor),
            ExprKind::Paren(e) => e.visit(a, visitor),
            ExprKind::SizeOf(type_name) => type_name.visit(a, visitor),
            ExprKind::Struct(members) => members.visit(a, visitor),
            ExprKind::Unop { e, .. } => e.visit(a, visitor),
            ExprKind::Ident { .. }
            | ExprKind::LiteralBool(_)
            | ExprKind::LiteralInt(_)
            | ExprKind::LiteralFloat(_)
            | ExprKind::LiteralString(_) => ControlFlow::Continue(()),
        }
    }
}

impl<'ast, V: Visitor<'ast, State = Analysis>> Analyzer<'ast, V>
    for TypeExpressionAnalyzer<'ast, V>
{
    fn visit(&self, visitor: &V, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<V::Break> {
        match node {
            Node::DefEnumConstant(def) => def.value.visit(a, visitor),
            Node::Expr(e) => self.expr_node(visitor, a, e),
            Node::FormalParam(param) => param.type_name.visit(a, visitor),
            Node::StructExprMember(member) => member.value.visit(a, visitor),
            Node::StructTypeMember(member) => {
                member.size.visit(a, visitor)?;
                member.type_name.visit(a, visitor)
            }
            Node::TypeName(type_name) => match &type_name.kind {
                TypeNameKind::String(size) => size.visit(a, visitor),
                TypeNameKind::Bool
                | TypeNameKind::Floating(_)
                | TypeNameKind::Integer(_)
                | TypeNameKind::QualIdent(_) => ControlFlow::Continue(()),
            },
            Node::DefAction(def) => def.type_name.visit(a, visitor),
            Node::DefAliasType(def) => def.type_name.visit(a, visitor),
            Node::DefArray(def) => {
                def.size.visit(a, visitor)?;
                def.elt_type.visit(a, visitor)?;
                def.default.visit(a, visitor)
            }
            Node::DefComponentInstance(def) => {
                def.base_id.visit(a, visitor)?;
                def.queue_size.visit(a, visitor)?;
                def.stack_size.visit(a, visitor)?;
                def.priority.visit(a, visitor)?;
                def.cpu.visit(a, visitor)?;
                def.init_specs.visit(a, visitor)
            }
            Node::DefConstant(def) => def.value.visit(a, visitor),
            Node::DefEnum(def) => {
                def.type_name.visit(a, visitor)?;
                def.constants.visit(a, visitor)?;
                def.default.visit(a, visitor)
            }
            Node::DefGuard(def) => def.type_name.visit(a, visitor),
            Node::DefPort(def) => {
                def.params.visit(a, visitor)?;
                def.return_type.visit(a, visitor)
            }
            Node::DefSignal(def) => def.type_name.visit(a, visitor),
            Node::DefStruct(def) => {
                def.members.visit(a, visitor)?;
                def.default.visit(a, visitor)
            }
            Node::SpecCommand(spec) => {
                spec.params.visit(a, visitor)?;
                spec.opcode.visit(a, visitor)?;
                spec.priority.visit(a, visitor)
            }
            Node::SpecDirectConnectionGraph(spec) => spec.connections.visit(a, visitor),
            Node::Connection(connection) => {
                connection.from_index.visit(a, visitor)?;
                connection.to_index.visit(a, visitor)
            }
            Node::SpecContainer(spec) => {
                spec.id.visit(a, visitor)?;
                spec.default_priority.visit(a, visitor)
            }
            Node::SpecEvent(spec) => {
                spec.params.visit(a, visitor)?;
                spec.id.visit(a, visitor)?;
                spec.throttle.visit(a, visitor)
            }
            Node::EventThrottle(throttle) => {
                throttle.count.visit(a, visitor)?;
                throttle.every.visit(a, visitor)
            }
            Node::SpecInit(spec) => spec.phase.visit(a, visitor),
            Node::SpecInternalPort(spec) => {
                spec.params.visit(a, visitor)?;
                spec.priority.visit(a, visitor)
            }
            Node::SpecParam(spec) => {
                spec.type_name.visit(a, visitor)?;
                spec.default.visit(a, visitor)?;
                spec.id.visit(a, visitor)?;
                spec.set_opcode.visit(a, visitor)?;
                spec.save_opcode.visit(a, visitor)
            }
            Node::SpecGeneralPortInstance(spec) => {
                spec.size.visit(a, visitor)?;
                spec.priority.visit(a, visitor)
            }
            Node::SpecSpecialPortInstance(spec) => spec.priority.visit(a, visitor),
            Node::SpecRecord(spec) => {
                spec.record_type.visit(a, visitor)?;
                spec.id.visit(a, visitor)
            }
            Node::SpecStateMachineInstance(spec) => spec.priority.visit(a, visitor),
            Node::SpecTlmChannel(spec) => {
                spec.type_name.visit(a, visitor)?;
                spec.id.visit(a, visitor)?;
                spec.low.visit(a, visitor)?;
                spec.high.visit(a, visitor)
            }
            Node::TlmChannelLimit(limit) => limit.value.visit(a, visitor),
            Node::SpecTlmPacket(spec) => {
                spec.id.visit(a, visitor)?;
                spec.group.visit(a, visitor)
            }
            Node::DefModuleTemplate(def) => {
                for param in &def.params {
                    if let TemplateParam::Constant(param) = param {
                        param.type_name.visit(a, visitor)?;
                    }
                }
                ControlFlow::Continue(())
            }
            _ => self.super_.visit(visitor, a, node),
        }
    }
}
