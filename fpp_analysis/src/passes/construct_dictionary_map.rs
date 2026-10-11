use crate::Analysis;
use crate::analyzers::module_analyzer::visit_expansion;
use crate::errors::SemanticResult;
use crate::semantics::{
    Dictionary, DictionaryEntries, DictionaryUsedSymbols, TlmPacket, TlmPacketSet,
};
use fpp_ast::{
    DefModule, DefTopology, SpecTemplateExpand, SpecTlmPacket, SpecTlmPacketSet, Visitable,
    Visitor, Walkable,
};
use std::ops::ControlFlow;
use std::sync::Arc;

/// Construct the dictionary map
pub struct ConstructDictionaryMap;

impl ConstructDictionaryMap {
    /// Construct a telemetry packet and add it to the packet set under
    /// construction
    fn add_packet(a: &mut Analysis, node: &SpecTlmPacket) -> SemanticResult {
        let id_opt = a.get_nonnegative_big_int_value_opt(&node.id)?;
        let packet = TlmPacket::from_spec_tlm_packet(
            a,
            a.dictionary
                .as_ref()
                .expect("dictionary under construction"),
            a.topology.as_ref().expect("topology under construction"),
            node,
        )?;
        a.tlm_packet_set
            .as_mut()
            .expect("packet set under construction")
            .add_packet(id_opt, packet)
    }

    /// Complete the packet set under construction and add it to the dictionary
    /// under construction
    fn add_packet_set(a: &mut Analysis, tpg: TlmPacketSet) -> SemanticResult {
        let tpg = tpg.complete(
            a,
            a.dictionary
                .as_ref()
                .expect("dictionary under construction"),
            a.topology.as_ref().expect("topology under construction"),
        )?;
        a.dictionary
            .as_mut()
            .expect("dictionary under construction")
            .add_tlm_packet_set(tpg)
    }
}

impl<'ast> Visitor<'ast> for ConstructDictionaryMap {
    type Break = ();
    type State = Analysis;

    fn visit_def_module(
        &self,
        a: &mut Self::State,
        node: &'ast DefModule,
    ) -> ControlFlow<Self::Break> {
        node.walk(a, self)
    }

    fn visit_spec_template_expand(
        &self,
        a: &mut Self::State,
        node: &'ast SpecTemplateExpand,
    ) -> ControlFlow<Self::Break> {
        visit_expansion(self, a, node)
    }

    fn visit_def_topology(
        &self,
        a: &mut Self::State,
        node: &'ast DefTopology,
    ) -> ControlFlow<Self::Break> {
        if !node.is_deployment {
            return ControlFlow::Continue(());
        }
        let Some(symbol) = a.get_symbol(node) else {
            return ControlFlow::Continue(());
        };
        // A topology that failed to resolve has no entry
        let Some(t) = a.topology_map.get(&symbol).cloned() else {
            return ControlFlow::Continue(());
        };
        let d = {
            let mut d = Dictionary::default();
            DictionaryUsedSymbols::new(&t).update_used_symbols(a, &mut d);
            DictionaryEntries::new(a, &t).update_entries(&mut d);
            d
        };
        a.topology = Some(t);
        a.dictionary = Some(d);
        let result = node.walk(a, self);
        let d = a.dictionary.take().expect("dictionary under construction");
        a.topology = None;
        result?;
        a.dictionary_map.insert(symbol, d);
        ControlFlow::Continue(())
    }

    fn visit_spec_tlm_packet_set(
        &self,
        a: &mut Self::State,
        node: &'ast SpecTlmPacketSet,
    ) -> ControlFlow<Self::Break> {
        a.tlm_packet_set = Some(TlmPacketSet::new(Arc::new(node.clone())));
        // A packet that fails to construct stops the construction of its set.
        // Its error is already reported, and construction goes on with the
        // next set.
        let members = node.members.visit(a, self);
        let tpg = a
            .tlm_packet_set
            .take()
            .expect("packet set under construction");
        if members.is_continue()
            && let Err(err) = Self::add_packet_set(a, tpg)
        {
            err.emit();
        }
        ControlFlow::Continue(())
    }

    fn visit_spec_tlm_packet(
        &self,
        a: &mut Self::State,
        node: &'ast SpecTlmPacket,
    ) -> ControlFlow<Self::Break> {
        match Self::add_packet(a, node) {
            Ok(()) => ControlFlow::Continue(()),
            Err(err) => {
                err.emit();
                ControlFlow::Break(())
            }
        }
    }
}
