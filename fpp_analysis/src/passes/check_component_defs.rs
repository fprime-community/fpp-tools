use crate::Analysis;
use crate::errors::SemanticResult;
use crate::semantics::{
    Command, Component, Container, Event, Param, PortInstance, Record, StateMachineInstance,
    Symbol, TlmChannel,
};
use fpp_ast::{
    AstNode, DefComponent, DefModule, SpecCommand, SpecContainer, SpecEvent,
    SpecGeneralPortInstance, SpecInterfaceImport, SpecInternalPort, SpecParam, SpecPortMatching,
    SpecRecord, SpecSpecialPortInstance, SpecStateMachineInstance, SpecTlmChannel, Visitor,
    Walkable,
};
use fpp_core::Spanned;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Check component definitions.
pub struct CheckComponentDefs;

/// Run a builder that produces an updated component, emitting any error and
/// marking the component as broken (so subsequent members are skipped) on error.
fn update(a: &mut Analysis, f: impl FnOnce(&Analysis, &mut Component) -> SemanticResult<()>) {
    let Some(mut component) = a.component.take() else {
        return;
    };
    match f(a, &mut component) {
        Ok(_) => a.component = Some(component),
        Err(err) => err.emit(),
    }
}

impl<'ast> Visitor<'ast> for CheckComponentDefs {
    type Break = ();
    type State = Analysis;

    fn visit_def_module(
        &self,
        a: &mut Self::State,
        node: &'ast DefModule,
    ) -> ControlFlow<Self::Break> {
        node.walk(a, self)
    }

    fn visit_def_component(
        &self,
        a: &mut Self::State,
        node: &'ast DefComponent,
    ) -> ControlFlow<Self::Break> {
        let symbol = a.get_symbol(node);
        let saved = a.component.take();
        a.component = Some(Component::new(symbol.clone(), Arc::new(node.clone())));
        node.walk(a, self)?;
        if let Some(component) = a.component.take() {
            match component.complete() {
                Ok(c) => {
                    a.component_map.insert(symbol, c);
                }
                Err(err) => err.emit(),
            }
        }
        a.component = saved;
        ControlFlow::Continue(())
    }

    fn visit_spec_command(
        &self,
        a: &mut Self::State,
        node: &'ast SpecCommand,
    ) -> ControlFlow<Self::Break> {
        update(a, |a, component| {
            let opcode = a.get_nonnegative_big_int_value_opt(&node.opcode)?;
            let command = Command::from_spec_command(a, node)?;
            component.add_command(opcode, command)
        });
        ControlFlow::Continue(())
    }

    fn visit_spec_container(
        &self,
        a: &mut Self::State,
        node: &'ast SpecContainer,
    ) -> ControlFlow<Self::Break> {
        update(a, |a, component| {
            let id = a.get_nonnegative_big_int_value_opt(&node.id)?;
            let container = Container::from_spec(a, node)?;
            component.add_container(id, container)
        });
        ControlFlow::Continue(())
    }

    fn visit_spec_event(
        &self,
        a: &mut Self::State,
        node: &'ast SpecEvent,
    ) -> ControlFlow<Self::Break> {
        update(a, |a, component| {
            let id = a.get_nonnegative_big_int_value_opt(&node.id)?;
            let event = Event::from_spec(a, node)?;
            component.add_event(id, event)
        });
        ControlFlow::Continue(())
    }

    fn visit_spec_internal_port(
        &self,
        a: &mut Self::State,
        node: &'ast SpecInternalPort,
    ) -> ControlFlow<Self::Break> {
        update(a, |a, component| {
            let instance = PortInstance::from_internal(a, node)?;
            component.add_port_instance(instance)
        });
        ControlFlow::Continue(())
    }

    fn visit_spec_param(
        &self,
        a: &mut Self::State,
        node: &'ast SpecParam,
    ) -> ControlFlow<Self::Break> {
        update(a, |a, component| {
            let id = a.get_nonnegative_big_int_value_opt(&node.id)?;
            let (param, default_opcode) = Param::from_spec(a, node, component.default_opcode)?;
            component.default_opcode = default_opcode;
            component.add_param(id, param)
        });
        ControlFlow::Continue(())
    }

    fn visit_spec_general_port_instance(
        &self,
        a: &mut Self::State,
        node: &'ast SpecGeneralPortInstance,
    ) -> ControlFlow<Self::Break> {
        update(a, |a, component| {
            let instance = PortInstance::from_general(a, node)?;
            component.add_port_instance(instance)
        });
        ControlFlow::Continue(())
    }

    fn visit_spec_special_port_instance(
        &self,
        a: &mut Self::State,
        node: &'ast SpecSpecialPortInstance,
    ) -> ControlFlow<Self::Break> {
        update(a, |a, component| {
            let instance = PortInstance::from_special(a, node)?;
            component.add_port_instance(instance)
        });
        ControlFlow::Continue(())
    }

    fn visit_spec_interface_import(
        &self,
        a: &mut Self::State,
        node: &'ast SpecInterfaceImport,
    ) -> ControlFlow<Self::Break> {
        update(a, |a, component| {
            let symbol = match a.use_def_map.get(&node.interface.id()) {
                Some(symbol @ Symbol::Interface(_)) => symbol.clone(),
                _ => return Ok(()),
            };
            let interface = match a.interface_map.get(&symbol) {
                Some(iface) => iface.clone(),
                None => return Ok(()),
            };
            component.add_imported_interface(&interface, node.span())
        });
        ControlFlow::Continue(())
    }

    fn visit_spec_port_matching(
        &self,
        a: &mut Self::State,
        node: &'ast SpecPortMatching,
    ) -> ControlFlow<Self::Break> {
        update(a, |_a, component| {
            Ok(component.add_spec_port_matching(Arc::new(node.clone())))
        });
        ControlFlow::Continue(())
    }

    fn visit_spec_record(
        &self,
        a: &mut Self::State,
        node: &'ast SpecRecord,
    ) -> ControlFlow<Self::Break> {
        update(a, |a, component| {
            let id = a.get_nonnegative_big_int_value_opt(&node.id)?;
            let record = Record::from_spec(a, node)?;
            component.add_record(id, record)
        });
        ControlFlow::Continue(())
    }

    fn visit_spec_state_machine_instance(
        &self,
        a: &mut Self::State,
        node: &'ast SpecStateMachineInstance,
    ) -> ControlFlow<Self::Break> {
        update(a, |a, component| {
            match StateMachineInstance::from_spec(a, node)? {
                Some(instance) => component.add_state_machine_instance(instance),
                None => Ok(()),
            }
        });
        ControlFlow::Continue(())
    }

    fn visit_spec_tlm_channel(
        &self,
        a: &mut Self::State,
        node: &'ast SpecTlmChannel,
    ) -> ControlFlow<Self::Break> {
        update(a, |a, component| {
            let id = a.get_nonnegative_big_int_value_opt(&node.id)?;
            let channel = TlmChannel::from_spec(a, node)?;
            component.add_tlm_channel(id, channel)
        });
        ControlFlow::Continue(())
    }
}
