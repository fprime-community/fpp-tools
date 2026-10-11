use std::fmt::{Display, Formatter};

/// A dependency on the F Prime framework
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameworkDependency {
    FwComp,
    FwCompQueued,
    Os,
}

impl Display for FrameworkDependency {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            FrameworkDependency::FwComp => "Fw_Comp",
            FrameworkDependency::FwCompQueued => "Fw_CompQueued",
            FrameworkDependency::Os => "Os",
        })
    }
}

impl FrameworkDependency {
    /// The position of a framework dependency in the sort order
    pub fn order_map(&self) -> usize {
        match self {
            FrameworkDependency::FwCompQueued => 0,
            FrameworkDependency::Os => 1,
            FrameworkDependency::FwComp => 2,
        }
    }

    /// Sorts a sequence of framework dependencies
    pub fn sort(s: &mut [FrameworkDependency]) {
        s.sort_by_key(FrameworkDependency::order_map);
    }
}
