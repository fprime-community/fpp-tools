use crate::Analysis;
use crate::analyzers::analyzer::{Analyzer, NestedScopeState};
use crate::analyzers::module_analyzer::visit_expansion;
use crate::analyzers::type_expression_analyzer::TypeExpressionAnalyzer;
use crate::semantics::{ImpliedUse, ImpliedUseKind, QualifiedName, Symbol};
use fpp_ast::*;
use std::ops::ControlFlow;

/// The use hooks of [`BasicUseAnalyzer`], and the hooks of its analysis of
/// template expansions
///
/// A pass gets these hooks called by running [`BasicUseAnalyzer`] or
/// [`super::UseAnalyzer`] as its super visitor. A hook receives the node of the
/// use and the qualified name the use names.
///
/// [`BasicUseAnalyzer`] analyzes the parameters on a template expansion through
/// the template hooks, where [`super::TemplateExpandAnalyzer`] visits the
/// arguments of the expansion specifier. The template argument hooks do
/// nothing by default. Visiting a bound argument needs a visitor for the
/// lifetime of the argument, which only a concrete pass can provide, so a pass
/// that analyzes the bound arguments overrides these hooks with
/// [`visit_template_arg`].
pub trait UseAnalysisPass<'ast, S: NestedScopeState>: Visitor<'ast, State = S> {
    /// A use of a component definition
    fn component_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = use_;
        ControlFlow::Continue(())
    }

    /// A use of a interface instance (topology def or component instance def)
    fn interface_instance_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = use_;
        ControlFlow::Continue(())
    }

    /// A use of a constant definition or enumerated constant definition
    ///
    /// The expression of an implied constant use is built for the visit, so
    /// `node` need not be part of the model.
    fn constant_use(
        &self,
        a: &mut Self::State,
        node: &Expr,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = use_;
        ControlFlow::Continue(())
    }

    /// A use of a port definition
    fn port_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = use_;
        ControlFlow::Continue(())
    }

    /// A use of an interface definition
    fn interface_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = use_;
        ControlFlow::Continue(())
    }

    /// A use of a type definition
    ///
    /// The type name of an implied type use is built for the visit, so `node`
    /// need not be part of the model.
    fn type_use(
        &self,
        a: &mut Self::State,
        node: &TypeName,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = use_;
        ControlFlow::Continue(())
    }

    /// A use of a state machine definition
    fn state_machine_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = use_;
        ControlFlow::Continue(())
    }

    /// A use of a template definition
    fn template_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = node;
        let _ = use_;
        ControlFlow::Continue(())
    }

    /// A use of a template constant parameter
    fn template_constant_arg(&self, a: &mut Self::State, arg: &Symbol) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = arg;
        ControlFlow::Continue(())
    }

    /// A use of a template type parameter
    fn template_type_arg(&self, a: &mut Self::State, arg: &Symbol) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = arg;
        ControlFlow::Continue(())
    }

    /// A use of a template interface parameter
    fn template_interface_arg(
        &self,
        a: &mut Self::State,
        arg: &Symbol,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = arg;
        ControlFlow::Continue(())
    }

    /// An implied constant use: a use of the constant definition named by the
    /// expression of the implied use
    fn implied_constant_use(
        &self,
        a: &mut Self::State,
        node: &Expr,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.constant_use(a, node, use_)
    }

    /// An implied port use: a use of the port definition named by the
    /// qualified identifier of the implied use
    fn implied_port_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.port_use(a, node, use_)
    }

    /// An implied type use: a use of the type definition named by the type
    /// name of the implied use
    fn implied_type_use(
        &self,
        a: &mut Self::State,
        node: &TypeName,
        use_: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.type_use(a, node, use_)
    }

    /// An implied use. It is visited after the use of the definition it names.
    fn implied_use(
        &self,
        a: &mut Self::State,
        iu: &ImpliedUse,
        kind: ImpliedUseKind,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = iu;
        let _ = kind;
        ControlFlow::Continue(())
    }

    /// A template argument of an expansion that has not been resolved.
    fn template_arg_node(
        &self,
        a: &mut Self::State,
        node: &'ast TemplateArg,
    ) -> ControlFlow<Self::Break> {
        match &node.kind {
            TemplateArgKind::Constant(e) => e.visit(a, self),
            TemplateArgKind::Type(tn) => tn.visit(a, self),
            TemplateArgKind::Interface(_) => ControlFlow::Continue(()),
        }
    }

    /// A bound template parameter of an expansion
    fn template_param(&self, a: &mut Self::State, arg: &Symbol) -> ControlFlow<Self::Break> {
        match arg {
            Symbol::TemplateConstantArg { .. } => self.template_constant_arg(a, arg),
            Symbol::TemplateTypeArg { .. } => self.template_type_arg(a, arg),
            Symbol::TemplateInterfaceArg { .. } => self.template_interface_arg(a, arg),
            _ => panic!("template parameter should be a template argument symbol"),
        }
    }
}

/// The use analysis of the argument bound to a template parameter: a use of a
/// template constant parameter visits the constant expression, a use of a
/// template type parameter the type name, and a use of a template interface
/// parameter is a use of a port interface instance.
///
/// This is the default analysis of the template argument hooks of
/// [`UseAnalysisPass`] for a use analysis.
pub fn visit_template_arg<'ast, S, V>(
    visitor: &V,
    a: &mut S,
    arg: &'ast Symbol,
) -> ControlFlow<V::Break>
where
    S: NestedScopeState,
    V: UseAnalysisPass<'ast, S>,
{
    match arg {
        Symbol::TemplateConstantArg { value, .. } => value.visit(a, visitor),
        Symbol::TemplateTypeArg { value, .. } => value.visit(a, visitor),
        Symbol::TemplateInterfaceArg { value, .. } => {
            visitor.interface_instance_use(a, value, value.as_ref().into())
        }
        _ => ControlFlow::Continue(()),
    }
}

/// Basic use analysis
/// Assumes all qualified identifiers are constant uses
pub struct BasicUseAnalyzer<'ast, V: UseAnalysisPass<'ast, Analysis>> {
    super_: TypeExpressionAnalyzer<'ast, V>,
}

impl<'ast, V: UseAnalysisPass<'ast, Analysis>> Default for BasicUseAnalyzer<'ast, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast, V: UseAnalysisPass<'ast, Analysis>> BasicUseAnalyzer<'ast, V> {
    pub fn new() -> BasicUseAnalyzer<'ast, V> {
        BasicUseAnalyzer {
            super_: TypeExpressionAnalyzer::new(),
        }
    }

    /// An implied constant use
    fn implied_constant_use(
        &self,
        visitor: &V,
        a: &mut Analysis,
        iu: &ImpliedUse,
    ) -> ControlFlow<V::Break> {
        iu.annotate_emitted_errors(|| {
            visitor.implied_constant_use(a, &iu.as_expr_node(), iu.name().clone())?;
            visitor.implied_use(a, iu, ImpliedUseKind::Constant)
        })
    }

    /// An implied port use
    fn implied_port_use(
        &self,
        visitor: &V,
        a: &mut Analysis,
        iu: &ImpliedUse,
    ) -> ControlFlow<V::Break> {
        iu.annotate_emitted_errors(|| {
            visitor.implied_port_use(a, &iu.as_qual_ident_node(), iu.name().clone())?;
            visitor.implied_use(a, iu, ImpliedUseKind::Port)
        })
    }

    /// An implied type use
    fn implied_type_use(
        &self,
        visitor: &V,
        a: &mut Analysis,
        iu: &ImpliedUse,
    ) -> ControlFlow<V::Break> {
        iu.annotate_emitted_errors(|| {
            visitor.implied_type_use(a, &iu.as_type_name_node(), iu.name().clone())?;
            visitor.implied_use(a, iu, ImpliedUseKind::Type)
        })
    }

    fn port_instance_identifier_node(
        &self,
        visitor: &V,
        a: &mut Analysis,
        node: &PortInstanceIdentifier,
    ) -> ControlFlow<V::Break> {
        visitor.interface_instance_use(
            a,
            &node.interface_instance,
            (&node.interface_instance).into(),
        )
    }

    fn tlm_channel_identifier_node(
        &self,
        visitor: &V,
        a: &mut Analysis,
        node: &TlmChannelIdentifier,
    ) -> ControlFlow<V::Break> {
        visitor.interface_instance_use(
            a,
            &node.component_instance,
            (&node.component_instance).into(),
        )
    }

    fn tlm_packet_member(
        &self,
        visitor: &V,
        a: &mut Analysis,
        member: &TlmPacketMember,
    ) -> ControlFlow<V::Break> {
        match member {
            TlmPacketMember::SpecInclude(_) => ControlFlow::Continue(()),
            TlmPacketMember::TlmChannelIdentifier(node) => {
                self.tlm_channel_identifier_node(visitor, a, node)
            }
        }
    }

    fn visit_implied_constant_uses(
        &self,
        visitor: &V,
        a: &mut Analysis,
        id: fpp_core::Node,
    ) -> ControlFlow<V::Break> {
        for iu in a.get_implied_uses(ImpliedUseKind::Constant, id) {
            self.implied_constant_use(visitor, a, &iu)?;
        }
        ControlFlow::Continue(())
    }

    fn visit_implied_port_uses(
        &self,
        visitor: &V,
        a: &mut Analysis,
        id: fpp_core::Node,
    ) -> ControlFlow<V::Break> {
        for iu in a.get_implied_uses(ImpliedUseKind::Port, id) {
            self.implied_port_use(visitor, a, &iu)?;
        }
        ControlFlow::Continue(())
    }

    fn visit_implied_type_uses(
        &self,
        visitor: &V,
        a: &mut Analysis,
        id: fpp_core::Node,
    ) -> ControlFlow<V::Break> {
        for iu in a.get_implied_uses(ImpliedUseKind::Type, id) {
            self.implied_type_use(visitor, a, &iu)?;
        }
        ControlFlow::Continue(())
    }

    fn visit_implied_uses(
        &self,
        visitor: &V,
        a: &mut Analysis,
        id: fpp_core::Node,
    ) -> ControlFlow<V::Break> {
        self.visit_implied_constant_uses(visitor, a, id)?;
        self.visit_implied_port_uses(visitor, a, id)?;
        self.visit_implied_type_uses(visitor, a, id)
    }
}

impl<'ast, V: UseAnalysisPass<'ast, Analysis>> Analyzer<'ast, V> for BasicUseAnalyzer<'ast, V> {
    fn visit(&self, visitor: &V, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<V::Break> {
        match node {
            Node::DefComponentInstance(ci) => {
                visitor.component_use(a, &ci.component, (&ci.component).into())?;
                self.super_.visit(visitor, a, node)
            }
            Node::DefSystem(s) => {
                visitor.interface_instance_use(a, &s.topology, (&s.topology).into())
            }
            Node::DefStateMachine(sm) => {
                self.visit_implied_uses(visitor, a, sm.node_id)?;
                self.super_.visit(visitor, a, node)
            }
            Node::DefTopology(t) => {
                self.visit_implied_uses(visitor, a, t.node_id)?;

                // Visit port interface uses in the implements clause
                for i in &t.implements {
                    visitor.interface_use(a, i, i.into())?;
                }

                self.super_.visit(visitor, a, node)
            }
            Node::Expr(e) => match &e.kind {
                ExprKind::Dot { e: e1, .. } => match e.get_ident_list_opt() {
                    // e is a qualified identifier
                    // Assume the entire qualified identifier is a use
                    Some(il) => visitor.constant_use(a, e, QualifiedName::from(il)),
                    // e is not a qualified identifier
                    // Analyze the left side, which may contain constant uses
                    None => e1.visit(a, visitor),
                },
                ExprKind::Ident { value, .. } => visitor.constant_use(a, e, value.clone().into()),
                _ => self.super_.visit(visitor, a, node),
            },
            Node::SpecInstance(si) => {
                visitor.interface_instance_use(a, &si.instance, (&si.instance).into())
            }
            Node::SpecStateMachineInstance(si) => {
                visitor.state_machine_use(a, &si.state_machine, (&si.state_machine).into())?;
                self.super_.visit(visitor, a, node)
            }
            Node::Connection(c) => {
                self.port_instance_identifier_node(visitor, a, &c.from_port)?;
                c.from_index.visit(a, visitor)?;
                self.port_instance_identifier_node(visitor, a, &c.to_port)?;
                c.to_index.visit(a, visitor)
            }
            Node::SpecPatternConnectionGraph(cg) => {
                visitor.interface_instance_use(a, &cg.source, (&cg.source).into())?;
                for target in &cg.targets {
                    visitor.interface_instance_use(a, target, target.into())?;
                }
                ControlFlow::Continue(())
            }
            Node::SpecGeneralPortInstance(pi) => {
                pi.size.visit(a, visitor)?;
                if let Some(port) = &pi.port {
                    visitor.port_use(a, port, port.into())?;
                }
                pi.priority.visit(a, visitor)
            }
            Node::SpecSpecialPortInstance(pi) => {
                pi.priority.visit(a, visitor)?;
                self.visit_implied_port_uses(visitor, a, pi.node_id)
            }
            Node::SpecTlmPacket(p) => {
                self.super_.visit(visitor, a, node)?;
                for member in &p.members {
                    self.tlm_packet_member(visitor, a, member)?;
                }
                ControlFlow::Continue(())
            }
            Node::SpecTlmPacketSet(s) => {
                self.super_.visit(visitor, a, node)?;
                for omitted in &s.omitted {
                    self.tlm_channel_identifier_node(visitor, a, omitted)?;
                }
                ControlFlow::Continue(())
            }
            Node::SpecInterfaceImport(i) => {
                visitor.interface_use(a, &i.interface, (&i.interface).into())
            }
            Node::SpecTopPort(p) => {
                self.port_instance_identifier_node(visitor, a, &p.underlying_port)
            }
            Node::TypeName(tn) => match &tn.kind {
                TypeNameKind::QualIdent(qi) => visitor.type_use(a, tn, qi.into()),
                TypeNameKind::String(_) => {
                    self.visit_implied_uses(visitor, a, tn.node_id)?;
                    self.super_.visit(visitor, a, node)
                }
                _ => self.super_.visit(visitor, a, node),
            },
            Node::SpecTemplateExpand(s) => {
                visitor.template_use(a, &s.template, (&s.template).into())?;

                // Analyze the parameters on this template expansion. An
                // expansion that has not been resolved has only its arguments.
                match a.template_expansion_params(s.node_id) {
                    Some(params) => {
                        for param in &params {
                            visitor.template_param(a, param)?;
                        }
                    }
                    None => {
                        for arg in &s.args {
                            visitor.template_arg_node(a, arg)?;
                        }
                    }
                }

                visit_expansion(visitor, a, s)
            }
            Node::DefModuleTemplate(d) => {
                // Visit the type of each constant parameter and the interface
                // of each interface parameter
                for param in &d.params {
                    match param {
                        TemplateParam::Constant(p) => p.type_name.visit(a, visitor)?,
                        TemplateParam::Interface(p) => {
                            visitor.interface_use(a, &p.interface, (&p.interface).into())?
                        }
                        TemplateParam::Type(_) => {}
                    }
                }
                ControlFlow::Continue(())
            }
            _ => self.super_.visit(visitor, a, node),
        }
    }
}
