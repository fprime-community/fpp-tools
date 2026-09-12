mod add_dependencies;
pub use add_dependencies::*;

mod compute_framework_dependencies;
pub use compute_framework_dependencies::*;

mod framework_dependency;
pub use framework_dependency::*;

use crate::Analysis;
use crate::passes::{BuildSpecLocMap, ConstructImpliedUseMap};
use fpp_ast::{MutVisitor, TransUnit, Visitor};
use fpp_core::{File, FileReader};
use std::ops::ControlFlow;

/// Compute dependencies for a list of translation units
pub struct ComputeDependencies<Reader: FileReader + Clone> {
    reader: Reader,
}

impl<Reader: FileReader + Clone> ComputeDependencies<Reader> {
    pub fn new(reader: Reader) -> ComputeDependencies<Reader> {
        ComputeDependencies { reader }
    }

    pub fn tu_list(&self, a: &mut Analysis, tul: &mut [&mut TransUnit]) -> ControlFlow<()> {
        a.level += 1;
        for tu in tul.iter_mut() {
            fpp_parser::ResolveIncludes::new(self.reader.clone())
                .visit_trans_unit(&mut a.include_context_map, tu)?;
        }
        a.included_file_set = a
            .include_context_map
            .keys()
            .map(|f| File::from_string(&f.uri()))
            .collect();
        if a.level == 1 {
            a.direct_dependency_file_set = a.included_file_set.clone();
        }
        let units: Vec<&TransUnit> = tul.iter().map(|tu| &**tu).collect();
        BuildSpecLocMap.visit_trans_units(a, units.iter().cloned())?;
        ConstructImpliedUseMap.visit_trans_units(a, units.iter().cloned())?;
        AddDependencies::new(self.reader.clone()).visit_trans_units(a, units.iter().cloned())?;
        let included_file_set = a.included_file_set.clone();
        a.dependency_file_set
            .retain(|f| !included_file_set.contains(f));
        a.level -= 1;
        ControlFlow::Continue(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ComputeDependencies, ComputeFrameworkDependencies, FrameworkDependency};
    use crate::Analysis;
    use fpp_core::{Error, File, FileReader, SourceFile};
    use rustc_hash::FxHashMap as HashMap;
    use std::rc::Rc;

    /// A reader backed by an in-memory path -> contents map
    #[derive(Clone)]
    struct MapReader {
        files: Rc<HashMap<String, String>>,
    }

    impl MapReader {
        fn new(files: &[(&str, &str)]) -> MapReader {
            MapReader {
                files: Rc::new(
                    files
                        .iter()
                        .map(|(p, c)| {
                            (
                                File::get_path(p).to_string_lossy().into_owned(),
                                c.to_string(),
                            )
                        })
                        .collect(),
                ),
            }
        }
    }

    impl FileReader for MapReader {
        fn read(&self, path: &str) -> Result<String, Error> {
            let path = File::get_path(path).to_string_lossy().into_owned();
            match self.files.get(&path) {
                Some(content) => Ok(content.clone()),
                None => Err(format!("cannot open {}", path).into()),
            }
        }
    }

    fn file(path: &str) -> File {
        File::from_string(path)
    }

    /// A location specifier for a defined symbol pulls in the file that defines
    /// it, transitively, and records it as a direct dependency at level 1.
    #[test]
    fn transitive_dependencies_are_collected() {
        let reader = MapReader::new(&[
            (
                "dep/a.fpp",
                "locate constant B at \"../dep/b.fpp\"\nconstant A = B\n",
            ),
            ("dep/b.fpp", "constant B = 1\n"),
        ]);
        let mut diagnostics = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
        fpp_core::run(&mut ctx, || {
            let src = "locate constant A at \"dep/a.fpp\"\nconstant C = A\n";
            let source = SourceFile::new("top.fpp", src.to_string());
            let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let mut a = Analysis::new();
            a.input_file_set.insert(file("top.fpp"));
            let _ = ComputeDependencies::new(reader.clone()).tu_list(&mut a, &mut [&mut ast]);

            let mut deps: Vec<String> = a
                .dependency_file_set
                .iter()
                .map(|f| f.to_string())
                .collect();
            deps.sort();
            assert_eq!(
                deps,
                vec![file("dep/a.fpp").to_string(), file("dep/b.fpp").to_string()]
            );
            // Only the file reached from the input files is direct.
            let direct: Vec<String> = a
                .direct_dependency_file_set
                .iter()
                .map(|f| f.to_string())
                .collect();
            assert_eq!(direct, vec![file("dep/a.fpp").to_string()]);
            assert!(a.missing_dependency_file_set.is_empty());
            // The crawl returns to level 0.
            assert_eq!(a.level, 0);
        });
    }

    /// A location specifier naming a file that cannot be opened is recorded as
    /// a missing dependency instead of failing the analysis.
    #[test]
    fn unopenable_dependency_is_recorded_as_missing() {
        let reader = MapReader::new(&[]);
        let mut diagnostics = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
        fpp_core::run(&mut ctx, || {
            let src = "locate constant A at \"missing.fpp\"\nconstant C = A\n";
            let source = SourceFile::new("top.fpp", src.to_string());
            let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let mut a = Analysis::new();
            a.input_file_set.insert(file("top.fpp"));
            let _ = ComputeDependencies::new(reader.clone()).tu_list(&mut a, &mut [&mut ast]);

            let missing: Vec<String> = a
                .missing_dependency_file_set
                .iter()
                .map(|f| f.to_string())
                .collect();
            assert_eq!(missing, vec![file("missing.fpp").to_string()]);
        });
    }

    /// A dictionary location specifier is pulled in only once a deployment
    /// topology has been seen.
    #[test]
    fn dictionary_dependencies_follow_the_first_deployment_topology() {
        let reader = MapReader::new(&[("dict.fpp", "dictionary constant D = 1\n")]);
        let mut diagnostics = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
        fpp_core::run(&mut ctx, || {
            // No deployment topology: the dictionary specifier is not followed.
            let src = "locate dictionary constant D at \"dict.fpp\"\ntopology T {\n}\n";
            let source = SourceFile::new("no_deployment.fpp", src.to_string());
            let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let mut a = Analysis::new();
            a.input_file_set.insert(file("no_deployment.fpp"));
            let _ = ComputeDependencies::new(reader.clone()).tu_list(&mut a, &mut [&mut ast]);
            assert!(!a.include_dictionary_deps);
            assert!(a.dependency_file_set.is_empty());

            // A deployment topology sets the flag and pulls the specifier in.
            let src = "locate dictionary constant D at \"dict.fpp\"\ndeployment topology T {\n}\n";
            let source = SourceFile::new("deployment.fpp", src.to_string());
            let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let mut a = Analysis::new();
            a.input_file_set.insert(file("deployment.fpp"));
            let _ = ComputeDependencies::new(reader.clone()).tu_list(&mut a, &mut [&mut ast]);
            assert!(a.include_dictionary_deps);
            let deps: Vec<String> = a
                .dependency_file_set
                .iter()
                .map(|f| f.to_string())
                .collect();
            assert_eq!(deps, vec![file("dict.fpp").to_string()]);
        });
    }

    /// A passive component depends on Fw_Comp; anything else depends on
    /// Fw_CompQueued and Os, and so does a guarded input port.
    #[test]
    fn framework_dependencies_follow_component_kind() {
        let mut diagnostics = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
        fpp_core::run(&mut ctx, || {
            let passive = "port P\npassive component C {\n  sync input port pIn: P\n}\n";
            let source = SourceFile::new("passive.fpp", passive.to_string());
            let ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let s = ComputeFrameworkDependencies::compute(&[&ast]);
            let mut deps: Vec<FrameworkDependency> = s.into_iter().collect();
            FrameworkDependency::sort(&mut deps);
            assert_eq!(deps, vec![FrameworkDependency::FwComp]);

            let guarded = "port P\npassive component C {\n  guarded input port pIn: P\n}\n";
            let source = SourceFile::new("guarded.fpp", guarded.to_string());
            let ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let s = ComputeFrameworkDependencies::compute(&[&ast]);
            let mut deps: Vec<FrameworkDependency> = s.into_iter().collect();
            FrameworkDependency::sort(&mut deps);
            assert_eq!(
                deps,
                vec![FrameworkDependency::Os, FrameworkDependency::FwComp]
            );

            let queued = "port P\nqueued component C {\n  sync input port pIn: P\n}\n";
            let source = SourceFile::new("queued.fpp", queued.to_string());
            let ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let s = ComputeFrameworkDependencies::compute(&[&ast]);
            let mut deps: Vec<FrameworkDependency> = s.into_iter().collect();
            FrameworkDependency::sort(&mut deps);
            assert_eq!(
                deps,
                vec![FrameworkDependency::FwCompQueued, FrameworkDependency::Os]
            );
            assert_eq!(
                FrameworkDependency::FwCompQueued.to_string(),
                "Fw_CompQueued"
            );
        });
    }
}
