use crate::analyzers::NestedScopeState;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::nested_analyzer::{NestedAnalyzer, NestedAnalyzerMode};
use crate::semantics::{ImpliedUse, QualifiedName};
use fpp_ast::*;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::ops::{ControlFlow, Deref};

/// An extension of the standard [Visitor] trait that allows analyzing uses of symbols
/// [BasicUseAnalyzer] or [super::UseAnalyzer] should be used in your pass for this to work properly
pub trait UseAnalysisPass<'ast, S: NestedScopeState>: Visitor<'ast, State = S> {
    /** A use of a component definition */
    fn component_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = name;
        ControlFlow::Continue(())
    }

    /** A use of an interface instance (topology def or component instance def) */
    fn interface_instance_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = name;
        ControlFlow::Continue(())
    }

    /** A use of a constant definition or enumerated constant definition */
    fn constant_use(
        &self,
        a: &mut Self::State,
        node: &'ast Expr,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = name;
        ControlFlow::Continue(())
    }

    /** A use of a port definition */
    fn port_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = name;
        ControlFlow::Continue(())
    }

    /** An implied use of a port definition (from a special port instance) */
    fn implied_port_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.port_use(a, node, name)
    }

    /** An implied use of a type definition (e.g. `FwSizeStoreType` for strings) */
    fn implied_type_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = name;
        ControlFlow::Continue(())
    }

    /** An implied use of a constant definition (e.g. `FW_FIXED_LENGTH_STRING_SIZE`
     * for default-size strings) */
    fn implied_constant_use(
        &self,
        a: &mut Self::State,
        node: &Expr,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = name;
        ControlFlow::Continue(())
    }

    /** A use of an interface definition */
    fn interface_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = name;
        ControlFlow::Continue(())
    }

    /** A use of a type definition */
    fn type_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = name;
        ControlFlow::Continue(())
    }

    /** A use of a state machine definition*/
    fn state_machine_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = name;
        ControlFlow::Continue(())
    }
}

pub struct BasicUseAnalyzer<'ast, S: NestedScopeState, V: UseAnalysisPass<'ast, S>> {
    super_: NestedAnalyzer<'ast, S, V>,
    phantom_data: PhantomData<S>,
}

impl<'ast, S: NestedScopeState, V: UseAnalysisPass<'ast, S>> Default
    for BasicUseAnalyzer<'ast, S, V>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast, S: NestedScopeState, V: UseAnalysisPass<'ast, S>> BasicUseAnalyzer<'ast, S, V> {
    pub fn new() -> BasicUseAnalyzer<'ast, S, V> {
        BasicUseAnalyzer {
            super_: NestedAnalyzer::new(NestedAnalyzerMode::DEEP),
            phantom_data: Default::default(),
        }
    }

    pub(crate) fn expr_to_qualified_name(&self, e: &Expr) -> Option<QualifiedName> {
        fn name_opt(e: &Expr, mut qualifier: VecDeque<String>) -> Option<QualifiedName> {
            match &e.kind {
                ExprKind::Ident(id) => {
                    qualifier.push_front(id.clone());
                    Some(qualifier.into())
                }
                ExprKind::Dot { e: e1, id } => {
                    qualifier.push_back(id.data.clone());
                    name_opt(e1.deref(), qualifier)
                }
                _ => None,
            }
        }

        name_opt(e, VecDeque::new())
    }
}

impl<'ast, S: NestedScopeState, V: UseAnalysisPass<'ast, S>> Analyzer<'ast, V>
    for BasicUseAnalyzer<'ast, S, V>
{
    fn visit(&self, visitor: &V, a: &mut V::State, node: Node<'ast>) -> ControlFlow<V::Break> {
        match node {
            Node::DefComponentInstance(ci) => {
                visitor.component_use(a, &ci.component, (&ci.component).into())?;
                ci.walk(a, visitor)
            }
            Node::DefTopology(t) => {
                for i in &t.implements {
                    visitor.interface_use(a, i, i.into())?;
                }

                t.walk(a, visitor)
            }
            Node::DefSystem(s) => {
                visitor.interface_instance_use(a, &s.topology, (&s.topology).into())?;
                s.walk(a, visitor)
            }
            Node::Expr(e) => match &e.kind {
                ExprKind::Dot { e: e1, .. } => {
                    match self.expr_to_qualified_name(e) {
                        // Assume the entire qualified identifier is a use
                        Some(use_) => visitor.constant_use(a, e, use_),
                        // This is some other type of dot expression (not a qual ident)
                        // Analyze the left side, which may contain constant uses
                        None => self.visit(visitor, a, Node::Expr(e1)),
                    }
                }
                ExprKind::Ident(id) => visitor.constant_use(a, e, id.clone().into()),
                _ => self.super_.visit(visitor, a, node),
            },
            Node::SpecInstance(si) => {
                visitor.interface_instance_use(a, &si.instance, (&si.instance).into())
            }
            Node::SpecStateMachineInstance(si) => {
                visitor.state_machine_use(a, &si.state_machine, (&si.state_machine).into())?;
                si.walk(a, visitor)
            }
            Node::SpecPatternConnectionGraph(cg) => {
                for target in &cg.targets {
                    visitor.interface_instance_use(a, target, target.into())?;
                }

                visitor.interface_instance_use(a, &cg.source, (&cg.source).into())?;

                self.super_.visit(visitor, a, node)
            }
            Node::SpecGeneralPortInstance(pi) => {
                match &pi.port {
                    None => ControlFlow::Continue(()),
                    Some(pqi) => visitor.port_use(a, pqi, pqi.into()),
                }?;

                self.super_.visit(visitor, a, node)
            }
            Node::SpecSpecialPortInstance(pi) => {
                let name = (match pi.kind {
                    SpecialPortInstanceKind::CommandRecv => "Cmd",
                    SpecialPortInstanceKind::CommandReg => "CmdReg",
                    SpecialPortInstanceKind::CommandResp => "CmdResponse",
                    SpecialPortInstanceKind::Event => "Log",
                    SpecialPortInstanceKind::ParamGet => "PrmGet",
                    SpecialPortInstanceKind::ParamSet => "PrmSet",
                    SpecialPortInstanceKind::ProductGet => "DpGet",
                    SpecialPortInstanceKind::ProductRecv => "DpResponse",
                    SpecialPortInstanceKind::ProductRequest => "DpRequest",
                    SpecialPortInstanceKind::ProductSend => "DpSend",
                    SpecialPortInstanceKind::Telemetry => "Tlm",
                    SpecialPortInstanceKind::TextEvent => "LogText",
                    SpecialPortInstanceKind::TimeGet => "Time",
                })
                .to_string();

                let port_qi = ImpliedUse::new(vec!["Fw".to_string(), name].into(), pi.node_id)
                    .as_qual_ident();
                visitor.implied_port_use(a, &port_qi, (&port_qi).into())?;
                self.super_.visit(visitor, a, node)
            }
            Node::PortInstanceIdentifier(pii) => visitor.interface_instance_use(
                a,
                &pii.interface_instance,
                (&pii.interface_instance).into(),
            ),
            Node::TlmChannelIdentifier(ci) => visitor.interface_instance_use(
                a,
                &ci.component_instance,
                (&ci.component_instance).into(),
            ),
            Node::SpecInterfaceImport(i) => {
                visitor.interface_use(a, &i.interface, (&i.interface).into())
            }
            Node::TypeName(tn) => match &tn.kind {
                TypeNameKind::QualIdent(qi) => visitor.type_use(a, qi, qi.into()),
                TypeNameKind::String(_) => {
                    // Dispatch the implied uses of the framework definitions
                    // required by a string type (populated by ConstructImpliedUseMap).
                    if let Some(uses) = a.implied_uses(tn.node_id) {
                        for iu in &uses.types {
                            let qi = iu.as_qual_ident();
                            visitor.implied_type_use(a, &qi, iu.name().clone())?;
                        }
                        for iu in &uses.constants {
                            let e = iu.as_expr();
                            visitor.implied_constant_use(a, &e, iu.name().clone())?;
                        }
                    }
                    self.super_.visit(visitor, a, node)
                }
                _ => ControlFlow::Continue(()),
            },
            _ => self.super_.visit(visitor, a, node),
        }
    }
}
