use fpp_macros::EnumMap;
use std::fmt::{Display, Formatter};

/// A name group
#[derive(EnumMap, Copy, Clone, Debug)]
pub enum NameGroup {
    PortInterfaceInstance,
    Component,
    Port,
    PortInterface,
    StateMachine,
    System,
    Topology,
    Type,
    Template,
    Value,
}

impl NameGroup {
    /// The name groups that a module definition resides in, in the order that
    /// a module symbol is entered into them. These are the name groups of the
    /// FPP spec (Scoping of Names, Name Groups); no definition resides in the
    /// topology name group.
    pub const GROUPS: [NameGroup; 9] = [
        NameGroup::Component,
        NameGroup::Port,
        NameGroup::StateMachine,
        NameGroup::PortInterfaceInstance,
        NameGroup::PortInterface,
        NameGroup::System,
        NameGroup::Template,
        NameGroup::Type,
        NameGroup::Value,
    ];
}

impl Display for NameGroup {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            NameGroup::PortInterfaceInstance => "component instance or topology",
            NameGroup::Component => "component",
            NameGroup::Port => "port",
            NameGroup::PortInterface => "interface",
            NameGroup::StateMachine => "state machine",
            NameGroup::System => "system",
            NameGroup::Topology => "topology",
            NameGroup::Type => "type",
            NameGroup::Template => "template",
            NameGroup::Value => "constant",
        };

        f.write_str(name)
    }
}
