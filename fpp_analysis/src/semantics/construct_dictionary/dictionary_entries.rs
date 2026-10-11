use crate::Analysis;
use crate::semantics::{
    CommandEntry, Component, ComponentInstance, ContainerEntry, Dictionary, EventEntry, ParamEntry,
    RecordEntry, TlmChannelEntry, Topology,
};
use indexmap::IndexMap;
use rustc_hash::FxBuildHasher;

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

    fn get_command_entry_map(&self) -> IndexMap<i128, CommandEntry, FxBuildHasher> {
        self.get_entry_map(
            |c| &c.command_map,
            |instance, command| CommandEntry { instance, command },
        )
    }

    fn get_tlm_channel_entry_map(&self) -> IndexMap<i128, TlmChannelEntry, FxBuildHasher> {
        self.get_entry_map(
            |c| &c.tlm_channel_map,
            |instance, tlm_channel| TlmChannelEntry {
                instance,
                tlm_channel,
            },
        )
    }

    fn get_event_entry_map(&self) -> IndexMap<i128, EventEntry, FxBuildHasher> {
        self.get_entry_map(
            |c| &c.event_map,
            |instance, event| EventEntry { instance, event },
        )
    }

    fn get_param_entry_map(&self) -> IndexMap<i128, ParamEntry, FxBuildHasher> {
        self.get_entry_map(
            |c| &c.param_map,
            |instance, param| ParamEntry { instance, param },
        )
    }

    fn get_record_entry_map(&self) -> IndexMap<i128, RecordEntry, FxBuildHasher> {
        self.get_entry_map(
            |c| &c.record_map,
            |instance, record| RecordEntry { instance, record },
        )
    }

    fn get_container_entry_map(&self) -> IndexMap<i128, ContainerEntry, FxBuildHasher> {
        self.get_entry_map(
            |c| &c.container_map,
            |instance, container| ContainerEntry {
                instance,
                container,
            },
        )
    }

    fn get_entry_map<S: Clone + 'a, E, M: 'a>(
        &self,
        get_spec_map: impl Fn(&'a Component) -> &'a M,
        construct_entry: impl Fn(ComponentInstance, S) -> E,
    ) -> IndexMap<i128, E, FxBuildHasher>
    where
        &'a M: IntoIterator<Item = (&'a i128, &'a S)>,
    {
        let add_entries_for_instance =
            |entry_map: &mut IndexMap<i128, E, FxBuildHasher>, ci: &ComponentInstance| {
                for (local_id, s) in get_spec_map(ci.component(self.a)) {
                    add_entry(entry_map, &construct_entry, ci, *local_id, s);
                }
            };
        let mut entry_map = IndexMap::default();
        for ci in self.t.component_instance_map().keys() {
            add_entries_for_instance(&mut entry_map, ci);
        }
        entry_map
    }
}

fn add_entry<S: Clone, E>(
    m: &mut IndexMap<i128, E, FxBuildHasher>,
    construct_entry: impl Fn(ComponentInstance, S) -> E,
    ci: &ComponentInstance,
    local_id: i128,
    s: &S,
) {
    let id = ci.base_id + local_id;
    let entry = construct_entry(ci.clone(), s.clone());
    m.insert(id, entry);
}
