use fpp_macros::EnumMap;
use std::fmt::{Display, Formatter};

/// A name group
#[derive(EnumMap, Copy, Clone, Debug)]
pub enum NameGroup {
    Component,
    Port,
    StateMachine,
    PortInterfaceInstance,
    PortInterface,
    System,
    Template,
    Type,
    Value,
}

impl NameGroup {
    /// The name groups a module symbol is entered into. A module is not in the
    /// system name group.
    pub const GROUPS: [NameGroup; 8] = [
        NameGroup::Component,
        NameGroup::Port,
        NameGroup::StateMachine,
        NameGroup::PortInterfaceInstance,
        NameGroup::PortInterface,
        NameGroup::Template,
        NameGroup::Type,
        NameGroup::Value,
    ];
}

impl Display for NameGroup {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            NameGroup::Component => "component",
            NameGroup::Port => "port",
            NameGroup::StateMachine => "state machine",
            NameGroup::PortInterfaceInstance => "component instance or topology",
            NameGroup::PortInterface => "interface",
            NameGroup::System => "system",
            NameGroup::Template => "template",
            NameGroup::Type => "type",
            NameGroup::Value => "constant",
        };

        f.write_str(name)
    }
}
