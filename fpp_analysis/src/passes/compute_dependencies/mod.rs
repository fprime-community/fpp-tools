mod add_dependencies;
pub use add_dependencies::*;

mod compute_framework_dependencies;
pub use compute_framework_dependencies::*;

mod framework_dependency;
pub use framework_dependency::*;

use crate::Analysis;
use crate::passes::{BuildSpecLocMap, ConstructImpliedUseMap};
use fpp_ast::{TransUnit, Visitor};
use fpp_core::{Error, FileReader, SourceFile};
use std::ops::ControlFlow;

/// Compute dependencies for a list of translation units
pub struct ComputeDependencies<'r, Reader: FileReader> {
    reader: &'r Reader,
}

impl<'r, Reader: FileReader> ComputeDependencies<'r, Reader> {
    pub fn new(reader: &'r Reader) -> ComputeDependencies<'r, Reader> {
        ComputeDependencies { reader }
    }

    /// Compute the dependencies of a list of translation units.
    ///
    /// The include specifiers are resolved in copies of the translation units,
    /// so the units passed in are unchanged.
    pub fn tu_list(&self, a: &mut Analysis, tul: &[TransUnit]) -> ControlFlow<()> {
        a.level += 1;
        let result = self.tu_list_at_level(a, tul);
        a.level -= 1;
        result
    }

    fn tu_list_at_level(&self, a: &mut Analysis, tul: &[TransUnit]) -> ControlFlow<()> {
        let mut tul = tul.to_vec();
        for tu in &mut tul {
            crate::resolve_includes(a, ReaderRef(self.reader), tu)?;
        }
        if a.level == 1 {
            a.direct_dependency_file_set = a.included_file_set.clone();
        }
        BuildSpecLocMap.visit_trans_units(a, tul.iter())?;
        ConstructImpliedUseMap.visit_trans_units(a, tul.iter())?;
        let expanded_template_names = CollectTemplateExpansions::names_in(tul.iter());
        AddDependencies::new(self.reader, expanded_template_names)
            .visit_trans_units(a, tul.iter())?;
        let included_file_set = a.included_file_set.clone();
        a.dependency_file_set
            .retain(|f| !included_file_set.contains(f));
        ControlFlow::Continue(())
    }
}

/// A borrowed [`FileReader`], for a [`fpp_parser::ResolveIncludes`], which takes
/// its reader by value
struct ReaderRef<'r, Reader: FileReader>(&'r Reader);

impl<Reader: FileReader> FileReader for ReaderRef<'_, Reader> {
    fn resolve(&self, current: SourceFile, include: &str) -> Result<String, Error> {
        self.0.resolve(current, include)
    }

    fn read(&self, path: &str) -> Result<String, Error> {
        self.0.read(path)
    }
}
