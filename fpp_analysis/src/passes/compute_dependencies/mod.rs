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
        let expanded_template_names = CollectTemplateExpansions::names_in(units.iter().cloned());
        AddDependencies::new(self.reader.clone(), expanded_template_names)
            .visit_trans_units(a, units.iter().cloned())?;
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
        /// Resolve relative to the directory of the including file. The default
        /// implementation canonicalizes that file, which the map's files do not
        /// have on disk.
        fn resolve(&self, current: SourceFile, include: &str) -> Result<String, Error> {
            let uri = current.uri();
            let path = match std::path::Path::new(&uri).parent() {
                Some(dir) => dir.join(include),
                None => std::path::PathBuf::from(include),
            };
            Ok(path.to_string_lossy().into_owned())
        }

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

    /// Runs `ComputeDependencies` over `inputs` and returns the sorted dependency
    /// file set.
    fn dependencies_of(reader: &MapReader, inputs: &[(&str, &str)]) -> Vec<String> {
        let mut diagnostics = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
        fpp_core::run(&mut ctx, || {
            let mut asts: Vec<fpp_ast::TransUnit> = inputs
                .iter()
                .map(|(path, src)| {
                    let source = SourceFile::new(path, src.to_string());
                    fpp_parser::parse(source, |p| p.trans_unit(), None)
                })
                .collect();
            let mut a = Analysis::new();
            for (path, _) in inputs {
                a.input_file_set.insert(file(path));
            }
            let mut refs: Vec<&mut fpp_ast::TransUnit> = asts.iter_mut().collect();
            let _ = ComputeDependencies::new(reader.clone()).tu_list(&mut a, &mut refs);
            let mut deps: Vec<String> = a
                .dependency_file_set
                .iter()
                .map(|f| f.to_string())
                .collect();
            deps.sort();
            deps
        })
    }

    /// A dotted constant use is looked up under its whole name, in source order
    /// (fpp-depend test `expr_dot`, plus a location for the name with its last two
    /// parts swapped).
    #[test]
    fn a_dotted_constant_use_depends_on_its_location() {
        let reader = MapReader::new(&[
            ("a.fpp", "module M { module N { constant a = 0 } }\n"),
            ("swapped.fpp", "module M { module a { constant N = 0 } }\n"),
        ]);
        let src = "\
locate constant M.N.a at \"a.fpp\"
locate constant M.a.N at \"swapped.fpp\"

constant b = M.N.a
";
        let deps = dependencies_of(&reader, &[("expr_dot.fpp", src)]);
        assert_eq!(deps, vec![file("a.fpp").to_string()]);
    }

    /// A qualified identifier of four parts is looked up under its whole name, in
    /// source order.
    #[test]
    fn a_four_part_type_use_depends_on_its_location() {
        let reader = MapReader::new(&[
            ("t.fpp", "module A { module B { module C { type T } } }\n"),
            (
                "swapped.fpp",
                "module A { module C { module B { type T } } }\n",
            ),
        ]);
        let src = "\
locate type A.B.C.T at \"t.fpp\"
locate type A.C.B.T at \"swapped.fpp\"

array X = [2] A.B.C.T
";
        let deps = dependencies_of(&reader, &[("qual_ident.fpp", src)]);
        assert_eq!(deps, vec![file("t.fpp").to_string()]);
    }

    /// The parameter list of a template, its expansion, and the arguments of the
    /// expansion are all sources of dependencies.
    #[test]
    fn template_parameters_and_arguments_are_dependencies() {
        let reader = MapReader::new(&[
            (
                "iface.fpp",
                "port P
interface ParamIface { sync input port pIn: P }
",
            ),
            (
                "body.fpp",
                "constant bodyConst = 7
",
            ),
            (
                "argtype.fpp",
                "type ArgType = U32
",
            ),
            (
                "arginst.fpp",
                "locate interface ParamIface at \"iface.fpp\"\npassive component ArgComp { import ParamIface }\ninstance argInst: ArgComp base id 0x200\n",
            ),
        ]);
        let src = "\
locate interface ParamIface at \"iface.fpp\"
locate constant bodyConst at \"body.fpp\"
locate type ArgType at \"argtype.fpp\"
locate instance argInst at \"arginst.fpp\"

module template T(instance i: ParamIface, type ty) {
  constant cOut = bodyConst
  struct S { x: ty }
  topology Top {
    instance i
  }
}

module M {
  expand T(instance argInst, type ArgType)
}
";
        let deps = dependencies_of(&reader, &[("top.fpp", src)]);
        assert_eq!(
            deps,
            vec![
                file("arginst.fpp").to_string(),
                file("argtype.fpp").to_string(),
                file("body.fpp").to_string(),
                file("iface.fpp").to_string(),
            ]
        );
    }

    /// An expansion depends on the file that defines the template it names.
    #[test]
    fn a_template_expansion_depends_on_the_template_definition() {
        let reader = MapReader::new(&[(
            "def.fpp",
            "module template T(constant p: U32) {
  constant cOut = p + 1
}
",
        )]);
        let src = "locate template T at \"def.fpp\"

module M {
  expand T(constant 10)
}
";
        let deps = dependencies_of(&reader, &[("top.fpp", src)]);
        assert_eq!(deps, vec![file("def.fpp").to_string()]);
    }

    /// The body of a template that is never expanded contributes no dependencies.
    #[test]
    fn an_unexpanded_template_body_contributes_no_dependencies() {
        let reader = MapReader::new(&[(
            "dep.fpp",
            "constant cycleConst = 1
",
        )]);
        let src = "\
locate constant cycleConst at \"dep.fpp\"

module template CycleT(constant p: U32) {
  constant cOut = p + cycleConst
}
";
        let deps = dependencies_of(&reader, &[("top.fpp", src)]);
        assert!(deps.is_empty(), "unexpected dependencies: {deps:?}");
    }

    /// The body of a template contributes dependencies when the template is
    /// expanded, even when the expansion is in another input file.
    #[test]
    fn a_template_body_contributes_dependencies_when_expanded_elsewhere() {
        let reader = MapReader::new(&[(
            "dep.fpp",
            "constant splitConst = 5
",
        )]);
        let def = "\
locate constant splitConst at \"dep.fpp\"

module template BodyLocSplitT(constant p: U32) {
  constant cOut = p + splitConst
}
";
        let use_ = "module M {
  expand BodyLocSplitT(constant 1)
}
";
        let deps = dependencies_of(&reader, &[("def.fpp", def), ("use.fpp", use_)]);
        assert_eq!(deps, vec![file("dep.fpp").to_string()]);
    }

    /// Below the input files the crawl cannot see the expansions, so it visits
    /// every template body. A body whose use leads back to an input file does not
    /// restart the crawl there.
    #[test]
    fn a_template_body_is_visited_below_the_input_files() {
        let reader = MapReader::new(&[
            (
                "def.fpp",
                "locate constant cycleConst at \"top.fpp\"
locate constant deepConst at \"deep.fpp\"

module template CycleT(constant p: U32) {
  constant cOut = p + cycleConst + deepConst
}
",
            ),
            (
                "deep.fpp",
                "constant deepConst = 2
",
            ),
        ]);
        let src = "\
locate template CycleT at \"def.fpp\"

constant cycleConst = 1

module M {
  expand CycleT(constant 2)
}
";
        let deps = dependencies_of(&reader, &[("top.fpp", src)]);
        assert_eq!(
            deps,
            vec![file("deep.fpp").to_string(), file("def.fpp").to_string()]
        );
    }

    /// An include specifier inside a template body is resolved like any other, so
    /// the crawl reports the included file.
    #[test]
    fn an_include_in_a_template_body_is_resolved() {
        let reader = MapReader::new(&[(
            "inc.fppi",
            "constant includedConst = 3
",
        )]);
        let mut diagnostics = vec![];
        let mut ctx =
            fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
        fpp_core::run(&mut ctx, || {
            let src = "\
module template IncT(constant p: U32) {
  include \"inc.fppi\"
  constant cOut = p + includedConst
}

module M {
  expand IncT(constant 1)
}
";
            let source = SourceFile::new("top.fpp", src.to_string());
            let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
            let mut a = Analysis::new();
            a.input_file_set.insert(file("top.fpp"));
            let _ = ComputeDependencies::new(reader.clone()).tu_list(&mut a, &mut [&mut ast]);

            let included: Vec<String> = a.included_file_set.iter().map(|f| f.to_string()).collect();
            assert_eq!(included, vec![file("inc.fppi").to_string()]);
            // The included constant is defined where it is used, so it locates
            // nothing and contributes no dependency of its own.
            assert!(a.dependency_file_set.is_empty());
        });
    }

    /// A deployment topology in an unexpanded template body does not pull in the
    /// dictionary dependencies.
    #[test]
    fn an_unexpanded_template_body_adds_no_dictionary_dependencies() {
        let reader = MapReader::new(&[
            (
                "dict_c.fpp",
                "dictionary constant c = 1
",
            ),
            (
                "dict_t.fpp",
                "dictionary type T1 = U32
",
            ),
        ]);
        let src = "\
locate dictionary constant c at \"dict_c.fpp\"
locate dictionary type T1 at \"dict_t.fpp\"

module template DictT(constant p: U32) {
  deployment topology DictTop { }
}
";
        let deps = dependencies_of(&reader, &[("top.fpp", src)]);
        assert!(deps.is_empty(), "unexpected dependencies: {deps:?}");
    }

    /// A string type used as a template type argument implies a use of the
    /// framework definitions that back it.
    #[test]
    fn a_string_type_argument_implies_the_framework_dependencies() {
        let reader = MapReader::new(&[(
            "dep.fpp",
            "type FwSizeStoreType = U32
constant FW_FIXED_LENGTH_STRING_SIZE = 256
",
        )]);
        let src = "\
locate type FwSizeStoreType at \"dep.fpp\"
locate constant FW_FIXED_LENGTH_STRING_SIZE at \"dep.fpp\"

module template StringArgT(type U) {
  struct StringArgS { s: U }
}

module M {
  expand StringArgT(type string)
}
";
        let deps = dependencies_of(&reader, &[("top.fpp", src)]);
        assert_eq!(deps, vec![file("dep.fpp").to_string()]);
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
