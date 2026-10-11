use crate::analyzers::NestedScopeState;
use crate::analyzers::analyzer::Analyzer;
use crate::analyzers::nested_analyzer::{NestedAnalyzer, NestedAnalyzerMode};
use crate::semantics::{ImpliedUse, ImpliedUseKind, QualifiedName, Symbol};
use fpp_ast::*;
use std::marker::PhantomData;
use std::ops::ControlFlow;

/// An extension of the standard [Visitor] trait that allows analyzing uses of symbols
/// [BasicUseAnalyzer] or [super::UseAnalyzer] should be used in your pass for this to work properly
pub trait UseAnalysisPass<'ast, S: NestedScopeState>: Visitor<'ast, State = S> {
    /// A use of a component definition
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

    /// A use of an interface instance (topology def or component instance def)
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

    /// A use of a constant definition or enumerated constant definition
    ///
    /// The expression of an implied constant use is built for the visit, so
    /// `node` need not be part of the model.
    fn constant_use(
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

    /// A use of a port definition
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

    /// An implied port use: a use of the port definition named by the
    /// qualified identifier of the implied use
    fn implied_port_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.port_use(a, node, name)
    }

    /// An implied type use: a use of the type definition named by the
    /// qualified identifier of the implied use
    fn implied_type_use(
        &self,
        a: &mut Self::State,
        node: &QualIdent,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.type_use(a, node, name)
    }

    /// An implied constant use: a use of the constant definition named by the
    /// expression of the implied use
    fn implied_constant_use(
        &self,
        a: &mut Self::State,
        node: &Expr,
        name: QualifiedName,
    ) -> ControlFlow<Self::Break> {
        self.constant_use(a, node, name)
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

    /// A use of an interface definition
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

    /// A use of a type definition
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

    /// A use of a state machine definition
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

    /// A use of a module template definition
    fn template_use(
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

    /// A use of a bound constant template parameter.
    ///
    /// A pass that analyzes the bound argument calls [`visit_template_arg`],
    /// which is the shared behavior these hooks build on.
    fn template_constant_arg(&self, a: &mut Self::State, arg: &Symbol) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = arg;
        ControlFlow::Continue(())
    }

    /// A use of a bound type template parameter
    fn template_type_arg(&self, a: &mut Self::State, arg: &Symbol) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = arg;
        ControlFlow::Continue(())
    }

    /// A use of a bound port interface instance template parameter
    fn template_interface_arg(
        &self,
        a: &mut Self::State,
        arg: &Symbol,
    ) -> ControlFlow<Self::Break> {
        let _ = a;
        let _ = arg;
        ControlFlow::Continue(())
    }

    /// A bound template parameter of an expansion
    fn template_param(&self, a: &mut Self::State, arg: &Symbol) -> ControlFlow<Self::Break> {
        match arg {
            Symbol::TemplateConstantArg { .. } => self.template_constant_arg(a, arg),
            Symbol::TemplateTypeArg { .. } => self.template_type_arg(a, arg),
            Symbol::TemplateInterfaceArg { .. } => self.template_interface_arg(a, arg),
            _ => ControlFlow::Continue(()),
        }
    }

    /// A template argument of an expansion that has not been resolved
    fn template_arg_node(
        &self,
        a: &mut Self::State,
        node: &'ast TemplateArg,
    ) -> ControlFlow<Self::Break> {
        match &node.kind {
            TemplateArgKind::Constant(e) => e.visit(a, self),
            TemplateArgKind::Type(tn) => tn.visit(a, self),
            // An unresolved interface argument names no definition yet
            TemplateArgKind::Interface(_) => ControlFlow::Continue(()),
        }
    }
}

/// The uses in the argument bound to a template parameter: a constant argument
/// is an expression, a type argument is a type name, and a port interface
/// instance argument is a use of a port interface instance.
///
/// This is the analysis a use pass performs on a bound parameter before doing
/// its own work with it, so it plays the role of the base-class behavior the
/// [`UseAnalysisPass`] template-argument hooks extend.
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

    /// An implied constant use
    fn implied_constant_use(
        &self,
        visitor: &V,
        a: &mut V::State,
        iu: &ImpliedUse,
    ) -> ControlFlow<V::Break> {
        iu.annotate_emitted_errors(|| {
            visitor.implied_constant_use(a, &iu.as_expr(), iu.name().clone())?;
            visitor.implied_use(a, iu, ImpliedUseKind::Constant)
        })
    }

    /// An implied port use
    fn implied_port_use(
        &self,
        visitor: &V,
        a: &mut V::State,
        iu: &ImpliedUse,
    ) -> ControlFlow<V::Break> {
        iu.annotate_emitted_errors(|| {
            visitor.implied_port_use(a, &iu.as_qual_ident(), iu.name().clone())?;
            visitor.implied_use(a, iu, ImpliedUseKind::Port)
        })
    }

    /// An implied type use
    fn implied_type_use(
        &self,
        visitor: &V,
        a: &mut V::State,
        iu: &ImpliedUse,
    ) -> ControlFlow<V::Break> {
        iu.annotate_emitted_errors(|| {
            visitor.implied_type_use(a, &iu.as_qual_ident(), iu.name().clone())?;
            visitor.implied_use(a, iu, ImpliedUseKind::Type)
        })
    }

    fn visit_implied_constant_uses(
        &self,
        visitor: &V,
        a: &mut V::State,
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
        a: &mut V::State,
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
        a: &mut V::State,
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
        a: &mut V::State,
        id: fpp_core::Node,
    ) -> ControlFlow<V::Break> {
        self.visit_implied_constant_uses(visitor, a, id)?;
        self.visit_implied_port_uses(visitor, a, id)?;
        self.visit_implied_type_uses(visitor, a, id)
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
                self.visit_implied_uses(visitor, a, t.node_id)?;

                // Visit port interface uses in the implements clause
                for i in &t.implements {
                    visitor.interface_use(a, i, i.into())?;
                }

                t.walk(a, visitor)
            }
            Node::DefStateMachine(sm) => {
                self.visit_implied_uses(visitor, a, sm.node_id)?;
                self.super_.visit(visitor, a, node)
            }
            Node::DefSystem(s) => {
                visitor.interface_instance_use(a, &s.topology, (&s.topology).into())?;
                s.walk(a, visitor)
            }
            Node::Expr(e) => match &e.kind {
                ExprKind::Dot { e: e1, .. } => {
                    match e.get_ident_list_opt() {
                        // e is a qualified identifier
                        // Assume the entire qualified identifier is a use
                        Some(il) => visitor.constant_use(a, e, QualifiedName::from(il)),
                        // e is not a qualified identifier
                        // Analyze the left side, which may contain constant uses
                        None => self.visit(visitor, a, Node::Expr(e1)),
                    }
                }
                ExprKind::Ident { value, .. } => visitor.constant_use(a, e, value.clone().into()),
                _ => self.super_.visit(visitor, a, node),
            },
            Node::SpecInstance(si) => {
                visitor.interface_instance_use(a, &si.instance, (&si.instance).into())
            }
            Node::DefModuleTemplate(d) => {
                // The body of a template is analyzed where the template is
                // expanded, not here. The parameter list is analyzed here,
                // because it does not depend on the body.
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
            Node::SpecTemplateExpand(s) => {
                visitor.template_use(a, &s.template, (&s.template).into())?;

                // Analyze the parameters bound by this expansion. Before the
                // expansion exists, all we have are the argument nodes.
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

                self.super_.visit(visitor, a, node)
            }
            Node::SpecStateMachineInstance(si) => {
                visitor.state_machine_use(a, &si.state_machine, (&si.state_machine).into())?;
                si.walk(a, visitor)
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
                    self.visit_implied_uses(visitor, a, tn.node_id)?;
                    self.super_.visit(visitor, a, node)
                }
                _ => ControlFlow::Continue(()),
            },
            _ => self.super_.visit(visitor, a, node),
        }
    }
}
