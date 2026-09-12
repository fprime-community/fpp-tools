use crate::errors::{SemanticError, SemanticResult};
use crate::semantics::{
    Command, ComponentInstance, Container, Event, Param, Record, Symbol, TlmChannel,
    TlmChannelIdentifier, TlmPacketSet, Topology,
};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::collections::BTreeMap;

/// An FPP dictionary
#[derive(Debug, Clone, Default)]
pub struct Dictionary {
    /// A set of symbols used in the dictionary
    pub used_symbol_set: HashSet<Symbol>,
    /// The map from global IDs to command entries
    pub command_entry_map: BTreeMap<CommandOpcode, CommandEntry>,
    /// The map from global IDs to telemetry channel entries
    pub tlm_channel_entry_map: BTreeMap<TlmChannelId, TlmChannelEntry>,
    /// The reverse telemetry channel map (for packet construction)
    pub reverse_tlm_channel_entry_map: HashMap<TlmChannelEntry, TlmChannelId>,
    /// The map from global IDs to event entries
    pub event_entry_map: BTreeMap<EventId, EventEntry>,
    /// The map from global IDs to parameter entries
    pub param_entry_map: BTreeMap<ParamId, ParamEntry>,
    /// The map from global IDs to record entries
    pub record_entry_map: BTreeMap<RecordId, RecordEntry>,
    /// The map from global IDs to container entries
    pub container_entry_map: BTreeMap<ContainerId, ContainerEntry>,
    /// The map from packet set names to packet sets
    pub tlm_packet_set_map: BTreeMap<String, TlmPacketSet>,
}

/// A command opcode
pub type CommandOpcode = i128;
/// A telemetry channel identifier
pub type TlmChannelId = i128;
/// An event identifier
pub type EventId = i128;
/// A parameter identifier
pub type ParamId = i128;
/// A record identifier
pub type RecordId = i128;
/// A container identifier
pub type ContainerId = i128;

impl Dictionary {
    /// Updates the reverse tlm channel entry map
    pub fn update_reverse_tlm_channel_entry_map(&mut self) {
        let mut map: HashMap<TlmChannelEntry, TlmChannelId> = HashMap::default();
        for (id, entry) in &self.tlm_channel_entry_map {
            map.insert(entry.clone(), *id);
        }
        self.reverse_tlm_channel_entry_map = map;
    }

    /// Finds the numeric ID for a telemetry channel identifier
    pub fn find_numeric_id_for_channel(
        &self,
        t: &Topology,
        channel_id: &TlmChannelIdentifier,
    ) -> SemanticResult<TlmChannelId> {
        let entry = TlmChannelEntry::from_tlm_channel_identifier(channel_id);
        match self.reverse_tlm_channel_entry_map.get(&entry) {
            Some(id) => Ok(*id),
            None => Err(SemanticError::ChannelNotInDictionary {
                loc: channel_id.get_loc(),
                channel_name: channel_id.get_qualified_name(),
                top_name: t.get_name().to_string(),
            }),
        }
    }

    /// Adds a telemetry packet set to the packet set map
    pub fn add_tlm_packet_set(&mut self, set: TlmPacketSet) -> SemanticResult {
        let name = set.get_name().to_string();
        match self.tlm_packet_set_map.get(&name) {
            Some(prev_group) => Err(SemanticError::DuplicateTlmPacketSet {
                name,
                loc: set.get_loc(),
                prev_loc: prev_group.get_loc(),
            }),
            None => {
                self.tlm_packet_set_map.insert(name, set);
                Ok(())
            }
        }
    }
}

/// A command entry in the dictionary
#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub instance: ComponentInstance,
    pub command: Command,
}

/// A container entry in the dictionary
#[derive(Debug, Clone)]
pub struct ContainerEntry {
    pub instance: ComponentInstance,
    pub container: Container,
}

/// A parameter entry in the dictionary
#[derive(Debug, Clone)]
pub struct ParamEntry {
    pub instance: ComponentInstance,
    pub param: Param,
}

/// A record entry in the dictionary
#[derive(Debug, Clone)]
pub struct RecordEntry {
    pub instance: ComponentInstance,
    pub record: Record,
}

/// A telemetry channel entry in the dictionary
#[derive(Debug, Clone)]
pub struct TlmChannelEntry {
    pub instance: ComponentInstance,
    pub tlm_channel: TlmChannel,
}

impl TlmChannelEntry {
    pub fn from_tlm_channel_identifier(identifier: &TlmChannelIdentifier) -> TlmChannelEntry {
        TlmChannelEntry {
            instance: identifier.component_instance.clone(),
            tlm_channel: identifier.tlm_channel.clone(),
        }
    }

    pub fn get_qualified_name(&self) -> String {
        format!(
            "{}.{}",
            self.instance.get_qualified_name(),
            self.tlm_channel.get_name()
        )
    }
}

impl PartialEq for TlmChannelEntry {
    fn eq(&self, other: &Self) -> bool {
        self.instance == other.instance
            && self.tlm_channel.get_name() == other.tlm_channel.get_name()
    }
}

impl Eq for TlmChannelEntry {}

impl std::hash::Hash for TlmChannelEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.instance.hash(state);
        self.tlm_channel.get_name().hash(state);
    }
}

/// An event entry in the dictionary
#[derive(Debug, Clone)]
pub struct EventEntry {
    pub instance: ComponentInstance,
    pub event: Event,
}

#[cfg(test)]
mod tests {
    use super::Dictionary;
    use crate::semantics::Symbol;
    use crate::{Analysis, add_state_enums, check_semantics};
    use fpp_core::SourceFile;

    /// Two instances of one component, each providing a command, an event, a
    /// parameter, two telemetry channels, a record and a container. The
    /// deployment topology declares one packet set that uses three channels and
    /// omits the fourth.
    const SRC: &str = r#"
module Fw {
  port Cmd
  port CmdReg
  port CmdResponse
  port Log
  port LogText
  port PrmGet
  port PrmSet
  port Time
  port Tlm
  port DpRequest
  port DpResponse
  port DpSend
}

dictionary type Payload = U32

active component C {

  command recv port cmdIn

  command reg port cmdRegOut

  command resp port cmdRespOut

  event port eventOut

  text event port textEventOut

  param get port prmGetOut

  param set port prmSetOut

  time get port timeGetOut

  telemetry port tlmOut

  product request port productRequestOut

  async product recv port productRecvIn

  product send port productSendOut

  async command CMD

  event EV severity activity high format "ev"

  param PRM: U32

  telemetry T1: Payload

  telemetry T2: U32

  product record R: U32

  product container CONT

}

instance c1: C base id 0x100 \
  queue size 10

instance c2: C base id 0x200 \
  queue size 10

deployment topology T {

  instance c1

  instance c2

  telemetry packets PK {

    packet Pa id 0 group 0 {
      c1.T1
      c1.T2
      c2.T1
    }

  } omit {
    c2.T2
  }

}
"#;

    /// Analyze [`SRC`] and hand the dictionary of topology `T` to `f`. Panics if
    /// the input produced any diagnostic.
    fn with_dictionary(f: impl FnOnce(&Analysis, &Dictionary)) {
        let mut diagnostics = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
        fpp_core::run(&mut ctx, || {
            let source = SourceFile::new("dictionary_test.fpp", SRC.to_string());
            let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let mut a = Analysis::new();
            add_state_enums(&mut ast);
            let _ = check_semantics(&mut a, vec![&ast]);
            let (_, dictionary) = a
                .dictionary_map
                .iter()
                .find(|(s, _)| matches!(s, Symbol::Topology(def) if def.name.data == "T"))
                .expect("topology T has a dictionary");
            f(&a, dictionary);
        });
        let output = String::from_utf8(diagnostics).expect("diagnostics are UTF-8");
        assert_eq!(output, "", "expected no diagnostics");
    }

    /// Every entry map is keyed by the global id: the instance base id plus the
    /// element's id within its component.
    #[test]
    fn entry_maps_are_keyed_by_global_id() {
        with_dictionary(|_, d| {
            let names = |ids: Vec<&i128>| ids.into_iter().copied().collect::<Vec<i128>>();
            // CMD plus the two commands implied by PRM, for each instance.
            assert_eq!(
                names(d.command_entry_map.keys().collect()),
                vec![0x100, 0x101, 0x102, 0x200, 0x201, 0x202]
            );
            assert_eq!(
                d.command_entry_map[&0x100].command.get_name(),
                "CMD".to_string()
            );
            assert_eq!(
                d.command_entry_map[&0x100].instance.get_qualified_name(),
                "c1"
            );
            assert_eq!(
                d.command_entry_map[&0x200].instance.get_qualified_name(),
                "c2"
            );

            assert_eq!(
                names(d.tlm_channel_entry_map.keys().collect()),
                vec![0x100, 0x101, 0x200, 0x201]
            );
            assert_eq!(
                d.tlm_channel_entry_map[&0x101].get_qualified_name(),
                "c1.T2"
            );

            assert_eq!(
                names(d.event_entry_map.keys().collect()),
                vec![0x100, 0x200]
            );
            assert_eq!(d.event_entry_map[&0x200].event.get_name(), "EV");

            assert_eq!(
                names(d.param_entry_map.keys().collect()),
                vec![0x100, 0x200]
            );
            assert_eq!(d.param_entry_map[&0x100].param.get_name(), "PRM");

            assert_eq!(
                names(d.record_entry_map.keys().collect()),
                vec![0x100, 0x200]
            );
            assert_eq!(d.record_entry_map[&0x100].record.get_name(), "R");

            assert_eq!(
                names(d.container_entry_map.keys().collect()),
                vec![0x100, 0x200]
            );
            assert_eq!(d.container_entry_map[&0x200].container.get_name(), "CONT");
        })
    }

    /// The reverse channel map inverts the channel entry map, so a channel
    /// identifier rebuilt from its instance and channel finds its global id.
    #[test]
    fn reverse_channel_map_inverts_the_channel_map() {
        with_dictionary(|_, d| {
            assert_eq!(
                d.reverse_tlm_channel_entry_map.len(),
                d.tlm_channel_entry_map.len()
            );
            for (id, entry) in &d.tlm_channel_entry_map {
                assert_eq!(d.reverse_tlm_channel_entry_map.get(entry), Some(id));
            }
        })
    }

    /// The packet set map holds the completed set, with its packet ids and the
    /// global ids of the channels it omits.
    #[test]
    fn packet_set_map_holds_the_completed_set() {
        with_dictionary(|_, d| {
            let set = d.tlm_packet_set_map.get("PK").expect("packet set PK");
            assert_eq!(set.packet_map.keys().collect::<Vec<&i128>>(), vec![&0]);
            assert_eq!(set.packet_map[&0].get_name(), "Pa");
            assert_eq!(set.packet_map[&0].member_id_list, vec![0x100, 0x101, 0x200]);
            assert_eq!(
                set.omitted_id_set.iter().copied().collect::<Vec<i128>>(),
                vec![0x201]
            );
        })
    }

    /// The used symbol set is the deep resolution of the symbols the dictionary
    /// elements use, together with the symbols marked with a dictionary
    /// specifier.
    #[test]
    fn used_symbol_set_resolves_uses() {
        with_dictionary(|a, d| {
            let mut names: Vec<String> = d
                .used_symbol_set
                .iter()
                .map(|s| a.get_qualified_name(s))
                .collect();
            names.sort();
            assert_eq!(names, vec!["Payload".to_string()]);
        })
    }
}
