use crate::Analysis;
use crate::semantics::{
    CommandEntry, Component, ComponentInstance, ContainerEntry, Dictionary, EventEntry, ParamEntry,
    RecordEntry, TlmChannelEntry, Topology,
};
use rustc_hash::FxHashMap as HashMap;
use std::collections::BTreeMap;

/// Fills in the dictionary entries
pub struct DictionaryEntries<'a> {
    a: &'a Analysis,
    t: &'a Topology,
}

impl<'a> DictionaryEntries<'a> {
    pub fn new(a: &'a Analysis, t: &'a Topology) -> DictionaryEntries<'a> {
        DictionaryEntries { a, t }
    }

    pub fn update_entries(&self, d: &mut Dictionary) {
        d.command_entry_map = self.get_command_entry_map();
        d.tlm_channel_entry_map = self.get_tlm_channel_entry_map();
        d.event_entry_map = self.get_event_entry_map();
        d.param_entry_map = self.get_param_entry_map();
        d.record_entry_map = self.get_record_entry_map();
        d.container_entry_map = self.get_container_entry_map();
        d.update_reverse_tlm_channel_entry_map();
    }

    fn get_command_entry_map(&self) -> BTreeMap<i128, CommandEntry> {
        self.get_entry_map(
            |c| &c.command_map,
            |instance, command| CommandEntry { instance, command },
        )
    }

    fn get_tlm_channel_entry_map(&self) -> BTreeMap<i128, TlmChannelEntry> {
        self.get_entry_map(
            |c| &c.tlm_channel_map,
            |instance, tlm_channel| TlmChannelEntry {
                instance,
                tlm_channel,
            },
        )
    }

    fn get_event_entry_map(&self) -> BTreeMap<i128, EventEntry> {
        self.get_entry_map(
            |c| &c.event_map,
            |instance, event| EventEntry { instance, event },
        )
    }

    fn get_param_entry_map(&self) -> BTreeMap<i128, ParamEntry> {
        self.get_entry_map(
            |c| &c.param_map,
            |instance, param| ParamEntry { instance, param },
        )
    }

    fn get_record_entry_map(&self) -> BTreeMap<i128, RecordEntry> {
        self.get_entry_map(
            |c| &c.record_map,
            |instance, record| RecordEntry { instance, record },
        )
    }

    fn get_container_entry_map(&self) -> BTreeMap<i128, ContainerEntry> {
        self.get_entry_map(
            |c| &c.container_map,
            |instance, container| ContainerEntry {
                instance,
                container,
            },
        )
    }

    fn get_entry_map<S: Clone, E>(
        &self,
        get_spec_map: impl Fn(&Component) -> &HashMap<i128, S>,
        construct_entry: impl Fn(ComponentInstance, S) -> E,
    ) -> BTreeMap<i128, E> {
        let mut entry_map = BTreeMap::new();
        for ci in self.t.component_instance_map().into_keys() {
            let Some(component) = ci.get_component(self.a) else {
                continue;
            };
            let mut ids: Vec<&i128> = get_spec_map(component).keys().collect();
            ids.sort();
            for local_id in ids {
                let s = get_spec_map(component)[local_id].clone();
                let id = ci.base_id + local_id;
                entry_map.insert(id, construct_entry(ci.clone(), s));
            }
        }
        entry_map
    }
}
