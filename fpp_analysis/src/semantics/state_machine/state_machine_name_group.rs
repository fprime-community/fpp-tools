use fpp_macros::EnumMap;
use std::fmt::{Display, Formatter};

/// A state machine name group
#[derive(EnumMap, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StateMachineNameGroup {
    Action,
    Guard,
    Signal,
    State,
}

impl Display for StateMachineNameGroup {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            StateMachineNameGroup::Action => "action",
            StateMachineNameGroup::Guard => "guard",
            StateMachineNameGroup::Signal => "signal",
            StateMachineNameGroup::State => "state",
        };
        f.write_str(name)
    }
}

impl StateMachineNameGroup {
    /// The list of all name groups
    pub const GROUPS: [StateMachineNameGroup; 4] = [
        StateMachineNameGroup::Action,
        StateMachineNameGroup::Guard,
        StateMachineNameGroup::Signal,
        StateMachineNameGroup::State,
    ];
}
