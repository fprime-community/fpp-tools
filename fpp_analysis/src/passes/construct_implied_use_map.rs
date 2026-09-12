use crate::Analysis;
use crate::semantics::{ImpliedUse, ImpliedUseKind, ImpliedUseSet, QualifiedName, Symbol};
use fpp_ast::{
    DefStateMachine, DefTopology, MoveWalkable, Node, SpecSpecialPortInstance,
    SpecialPortInstanceKind, TypeName, TypeNameKind, Visitor, Walkable,
};
use std::ops::ControlFlow;
use std::sync::Arc;

/// Construct the implied use map
pub struct ConstructImpliedUseMap;

/// The name of the framework port implied by a special port instance
pub fn special_port_implied_use_name(kind: &SpecialPortInstanceKind) -> &'static str {
    match kind {
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
    }
}

impl<'ast> Visitor<'ast> for ConstructImpliedUseMap {
    type Break = ();
    type State = Analysis;

    /// Descend into every container so all type names in the model are reached.
    fn super_visit(&self, a: &mut Analysis, node: Node<'ast>) -> ControlFlow<Self::Break> {
        node.walk(a, self)
    }

    fn visit_spec_special_port_instance(
        &self,
        a: &mut Analysis,
        node: &'ast SpecSpecialPortInstance,
    ) -> ControlFlow<Self::Break> {
        // Construct the port use implied by the special port instance
        let ident_list = vec![
            "Fw".to_string(),
            special_port_implied_use_name(&node.kind).to_string(),
        ];
        let mut set = ImpliedUseSet::default();
        set.ports.push(ImpliedUse::from_ident_list_and_id(
            ident_list,
            node.node_id,
            Vec::new(),
        ));
        a.implied_use_map.insert(node.node_id, set);
        self.super_visit(a, Node::SpecSpecialPortInstance(node))
    }

    fn visit_def_state_machine(
        &self,
        a: &mut Analysis,
        node: &'ast DefStateMachine,
    ) -> ControlFlow<Self::Break> {
        if node.members.is_some() {
            let symbol = a
                .get_symbol(node)
                .unwrap_or_else(|| Symbol::StateMachine(Arc::new(node.clone())));
            let mut idents: Vec<String> = a
                .get_qualified_name(&symbol)
                .split('.')
                .map(str::to_string)
                .collect();
            idents.push("State".to_string());
            let name: QualifiedName = idents.into();
            let mut set = ImpliedUseSet::default();
            set.types
                .push(ImpliedUse::from_name_and_id(name, node.node_id, Vec::new()));
            a.implied_use_map.insert(node.node_id, set);
        }
        self.super_visit(a, Node::DefStateMachine(node))
    }

    fn visit_def_topology(
        &self,
        a: &mut Analysis,
        node: &'ast DefTopology,
    ) -> ControlFlow<Self::Break> {
        if !node.is_deployment {
            return ControlFlow::Continue(());
        }
        let annotations =
            vec!["this implied use occurs when constructing a dictionary".to_string()];
        let mut set = ImpliedUseSet::default();
        for type_name in ImpliedUse::get_topology_types(a) {
            let id = ImpliedUse::replicate_id(node.node_id);
            set.add(
                ImpliedUseKind::Type,
                ImpliedUse::from_ident_list_and_id(type_name, id, annotations.clone()),
            );
        }
        for constant in ImpliedUse::get_topology_constants(a) {
            let id = ImpliedUse::replicate_id(node.node_id);
            set.add(
                ImpliedUseKind::Constant,
                ImpliedUse::from_ident_list_and_id(constant, id, annotations.clone()),
            );
        }
        a.implied_use_map.insert(node.node_id, set);
        node.walk(a, self)
    }

    fn visit_type_name(&self, a: &mut Analysis, node: &'ast TypeName) -> ControlFlow<Self::Break> {
        if let TypeNameKind::String(size) = &node.kind {
            let mut set = ImpliedUseSet::default();
            set.types.push(ImpliedUse::from_ident_list_and_id(
                vec!["FwSizeStoreType".to_string()],
                ImpliedUse::replicate_id(node.node_id),
                vec!["use of a string type requires this definition".to_string()],
            ));
            if size.is_none() {
                set.constants.push(ImpliedUse::from_ident_list_and_id(
                    vec!["FW_FIXED_LENGTH_STRING_SIZE".to_string()],
                    ImpliedUse::replicate_id(node.node_id),
                    vec![
                        "use of a string type with default size requires this definition"
                            .to_string(),
                    ],
                ));
            }
            a.implied_use_map.insert(node.node_id, set);
        }
        self.super_visit(a, Node::TypeName(node))
    }
}
