//! Tests of dependency analysis, run the way the Scala tool `fpp-depend` runs
//! it.
//!
//! The test inputs and the reference outputs in `fpp_depend/` are copies of
//! the Scala `fpp-depend` test suite. A reference output names a file in the
//! test directory with the prefix `[ local path prefix ]/compiler/tools/fpp-depend/test`,
//! as the Scala suite does.

use fpp_analysis::passes::{
    ComputeDependencies, ComputeFrameworkDependencies, EnterSymbols, FrameworkDependency,
};
use fpp_analysis::{Analysis, add_state_enums, resolve_includes, resolve_templates};
use fpp_ast::{TransUnit, Visitor};
use fpp_core::{DiagnosticData, DiagnosticEmitter, File, FileReader, Level, SourceFile};
use fpp_fs::FsReader;
use pretty_assertions::assert_eq;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{env, fs};

/// The prefix the reference outputs use for the test directory
const PATH_PREFIX: &str = "[ local path prefix ]/compiler/tools/fpp-depend/test";

/// The directory holding the test inputs and reference outputs
fn test_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("fpp_depend");
    path
}

/// An emitter that keeps each rendered diagnostic, so that the diagnostics of
/// an attempt that is abandoned can be dropped
#[derive(Clone, Default)]
struct BufferEmitter {
    diagnostics: Rc<RefCell<Vec<(Level, String)>>>,
}

impl BufferEmitter {
    fn len(&self) -> usize {
        self.diagnostics.borrow().len()
    }

    fn truncate(&self, len: usize) {
        self.diagnostics.borrow_mut().truncate(len)
    }

    fn has_errors_since(&self, len: usize) -> bool {
        self.diagnostics.borrow()[len..]
            .iter()
            .any(|(level, _)| matches!(level, Level::Error))
    }

    fn render(&self) -> String {
        self.diagnostics
            .borrow()
            .iter()
            .map(|(_, text)| text.as_str())
            .collect()
    }
}

impl DiagnosticEmitter for BufferEmitter {
    fn emit(&mut self, diagnostic: DiagnosticData) {
        let mut text =
            fpp_errors::OwnedDiagnostic::from(&diagnostic).render(&fpp_errors::renderer(false));
        text.push_str("\n\n");
        self.diagnostics.borrow_mut().push((diagnostic.level, text));
    }
}

/// The options of an `fpp-depend` run
#[derive(Default)]
struct Options {
    /// The input files, relative to the test directory
    files: Vec<String>,
    /// Write the direct dependencies to this file (`-d`)
    direct_file: Option<String>,
    /// Write the framework dependencies to this file (`-f`)
    framework_file: Option<String>,
    /// Write the included files to this file (`-i`)
    included_file: Option<String>,
    /// Write the missing dependencies to this file (`-m`)
    missing_file: Option<String>,
}

impl Options {
    /// Parse the options of a Scala test: `args` holds the flags, each followed
    /// by the file it names, and `infile` the input files without the
    /// extension of the last one
    fn parse(args: &str, infile: &str) -> Options {
        let mut options = Options::default();
        let mut words = args.split_whitespace();
        while let Some(flag) = words.next() {
            let file = Some(words.next().expect("an option names a file").to_string());
            match flag {
                "-d" => options.direct_file = file,
                "-f" => options.framework_file = file,
                "-i" => options.included_file = file,
                "-m" => options.missing_file = file,
                _ => panic!("unknown option {flag}"),
            }
        }
        options.files = format!("{infile}.fpp")
            .split_whitespace()
            .map(str::to_string)
            .collect();
        options
    }
}

/// The output of an `fpp-depend` run: standard output, and the contents of each
/// file named by an option
struct Output {
    stdout: String,
    files: Vec<(String, String)>,
}

/// Sort the names of a collection, one per line
fn map_iterable<T: ToString>(it: impl IntoIterator<Item = T>, sorting: bool) -> String {
    let mut lines: Vec<String> = it.into_iter().map(|x| x.to_string()).collect();
    if sorting {
        lines.sort();
    }
    lines.into_iter().map(|line| line + "\n").collect()
}

/// A new analysis of the input files
fn new_analysis(files: &[PathBuf]) -> Analysis {
    let mut a = Analysis::new();
    a.input_file_set = files
        .iter()
        .map(|f| File::from_string(&f.to_string_lossy()))
        .collect();
    a.dictionary_generation = true;
    a
}

/// Perform the syntax expansion and enter the symbols. If that fails, enter
/// the symbols of the unexpanded model, and if that fails too, go on without
/// symbols. The diagnostics of an attempt that fails are dropped.
fn expand_syntax_and_enter_symbols(
    emitter: &BufferEmitter,
    files: &[PathBuf],
    tul: Vec<TransUnit>,
) -> (Analysis, Vec<TransUnit>) {
    let mark = emitter.len();
    let mut a = new_analysis(files);
    let mut expanded = tul.clone();
    let mut result = expanded
        .iter_mut()
        .try_for_each(|tu| resolve_includes(&mut a, FsReader {}, tu));
    if result.is_continue() {
        expanded.iter_mut().for_each(add_state_enums);
        result = resolve_templates(&mut a, &mut expanded);
    }
    if result.is_continue() && !emitter.has_errors_since(mark) {
        return (a, expanded);
    }
    emitter.truncate(mark);
    let mut tul = tul;
    tul.iter_mut().for_each(add_state_enums);
    let mut a = new_analysis(files);
    let result = EnterSymbols.visit_trans_units(&mut a, tul.iter());
    if result.is_break() || emitter.has_errors_since(mark) {
        emitter.truncate(mark);
        a = new_analysis(files);
    }
    (a, tul)
}

/// Run `fpp-depend` with the given options, in the given directory
fn command(dir: &Path, options: &Options) -> Output {
    let files: Vec<PathBuf> = options.files.iter().map(|f| dir.join(f)).collect();
    let emitter = BufferEmitter::default();
    let mut ctx = fpp_core::CompilerContext::new(emitter.clone());
    let output = fpp_core::run(&mut ctx, || {
        let mut tul = vec![];
        for file in &files {
            let path = file.to_string_lossy();
            let src = FsReader {}
                .read(&path)
                .unwrap_or_else(|err| panic!("failed to open {path}: {err}"));
            tul.push(fpp_parser::parse(
                SourceFile::new(&path, src),
                |p| p.trans_unit(),
                None,
            ));
            if emitter.has_errors_since(0) {
                return None;
            }
        }
        let (mut a, tul) = expand_syntax_and_enter_symbols(&emitter, &files, tul);
        let _ = ComputeDependencies::new(&FsReader {}).tu_list(&mut a, &tul);
        if emitter.has_errors_since(0) {
            return None;
        }
        let mut side_files = vec![];
        if let Some(file) = &options.direct_file {
            side_files.push((
                file.clone(),
                map_iterable(&a.direct_dependency_file_set, true),
            ));
        }
        if let Some(file) = &options.framework_file {
            let mut ds = Default::default();
            let _ = ComputeFrameworkDependencies.visit_trans_units(&mut ds, tul.iter());
            let mut ds: Vec<FrameworkDependency> = ds.into_iter().collect();
            FrameworkDependency::sort(&mut ds);
            // Don't sort alphabetically
            // Already sorted by dependency order
            side_files.push((file.clone(), map_iterable(ds, false)));
        }
        if let Some(file) = &options.included_file {
            side_files.push((file.clone(), map_iterable(&a.included_file_set, true)));
        }
        if let Some(file) = &options.missing_file {
            side_files.push((
                file.clone(),
                map_iterable(&a.missing_dependency_file_set, true),
            ));
        }
        Some(Output {
            stdout: map_iterable(&a.dependency_file_set, true),
            files: side_files,
        })
    });
    drop(ctx);
    let output = output.unwrap_or_else(|| Output {
        stdout: emitter.render(),
        files: vec![],
    });
    let dir = dir.to_string_lossy().into_owned();
    Output {
        stdout: output.stdout.replace(&dir, PATH_PREFIX),
        files: output
            .files
            .into_iter()
            .map(|(file, contents)| (file, contents.replace(&dir, PATH_PREFIX)))
            .collect(),
    }
}

/// Compare an output with its reference output, or update the reference
/// output when `FPP_UPDATE_REF` is set
fn compare(name: &str, output: &str) {
    let ref_file = test_dir().join(format!("{name}.ref.txt"));
    match env::var("FPP_UPDATE_REF") {
        Ok(_) => fs::write(&ref_file, output).expect("failed to write ref.txt"),
        Err(_) => {
            let ref_txt = fs::read_to_string(&ref_file)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", ref_file.display()));
            assert_eq!(ref_txt, output, "{name}");
        }
    }
}

/// Run a test: `args` holds the options, `infile` the input files without the
/// extension of the last one, and `outfile` the name of the reference output
/// for standard output (`infile` when `None`). An option that names a file
/// `X.out.txt` compares that file with `X.ref.txt`.
fn run_test(args: &str, infile: &str, outfile: Option<&str>) {
    run_test_in(&test_dir(), args, infile, outfile)
}

/// Run a test in a directory other than the test directory, for example one
/// that names the test directory through a symbolic link. The outputs name the
/// files under that directory.
fn run_test_in(dir: &Path, args: &str, infile: &str, outfile: Option<&str>) {
    let options = Options::parse(args, infile);
    let output = command(dir, &options);
    compare(outfile.unwrap_or(infile), &output.stdout);
    for (file, contents) in &output.files {
        let name = file
            .strip_suffix(".out.txt")
            .expect("an option names an .out.txt file");
        compare(name, contents);
    }
}

#[test]
fn def_alias() {
    run_test("", "def_alias", None)
}

#[test]
fn def_array() {
    run_test("", "def_array", None)
}

#[test]
fn def_component_instance() {
    run_test("", "def_component_instance", None)
}

#[test]
fn def_constant() {
    run_test("", "def_constant", None)
}

#[test]
fn def_enum() {
    run_test("", "def_enum", None)
}

#[test]
fn def_interface() {
    run_test("", "def_interface", None)
}

#[test]
fn def_port() {
    run_test("", "def_port", None)
}

#[test]
fn def_state_machine() {
    run_test("", "def_state_machine", None)
}

#[test]
fn def_struct() {
    run_test("", "def_struct", None)
}

#[test]
fn def_system() {
    run_test("", "def_system", None)
}

#[test]
fn dictionary() {
    run_test("", "dictionary", None)
}

#[test]
fn dictionary_no_deployment_top() {
    run_test("", "dictionary_no_deployment_top", None)
}

#[test]
fn direct() {
    run_test("-d direct_output.out.txt", "direct_a", Some("direct"))
}

#[test]
fn enum_constant() {
    run_test("", "enum_constant", None)
}

#[test]
fn expr_array() {
    run_test("", "expr_array", None)
}

#[test]
fn expr_binop() {
    run_test("", "expr_binop", None)
}

#[test]
fn expr_dot() {
    run_test("", "expr_dot", None)
}

#[test]
fn expr_ident() {
    run_test("", "expr_ident", None)
}

#[test]
fn expr_paren() {
    run_test("", "expr_paren", None)
}

#[test]
fn expr_sizeof() {
    run_test("", "expr_sizeof", None)
}

#[test]
fn expr_sizeof_implied_type() {
    run_test("", "expr_sizeof_implied_type", None)
}

#[test]
fn expr_struct() {
    run_test("", "expr_struct", None)
}

#[test]
fn expr_struct_dot() {
    run_test("", "expr_struct_dot", None)
}

#[test]
fn expr_unop() {
    run_test("", "expr_unop", None)
}

// The Scala test also writes the names of the generated files (-g, -u), which
// the Rust port does not compute; only the dependencies are compared
#[test]
fn filenames() {
    run_test("", "fpp-filenames/ok", Some("filenames"))
}

// The Scala test also writes the names of the generated files (-a, -g, -u),
// which the Rust port does not compute; only the dependencies are compared
#[test]
fn filenames_auto() {
    run_test("", "fpp-filenames/ok", Some("filenames"))
}

// The Scala test also writes the names of the generated files (-g, -u), which
// the Rust port does not compute; only the dependencies are compared
#[test]
fn filenames_include() {
    run_test("", "fpp-filenames/include", Some("filenames_include"))
}

// The Scala test also writes the names of the generated files (-a, -g, -u),
// which the Rust port does not compute; only the dependencies are compared
#[test]
fn filenames_include_auto() {
    run_test("", "fpp-filenames/include", Some("filenames_include"))
}

#[test]
fn framework() {
    run_test("-f framework_output.out.txt", "framework", None)
}

#[test]
fn framework_include() {
    run_test(
        "-f framework_include_output.out.txt",
        "framework_include",
        None,
    )
}

#[test]
fn include() {
    run_test("-i include_output.out.txt", "include", None)
}

#[test]
fn included_dep() {
    run_test("", "included_dep", None)
}

#[test]
fn included_dep_i() {
    run_test(
        "-i included_dep_i_output.out.txt",
        "included_dep",
        Some("included_dep_i"),
    )
}

#[test]
fn input() {
    run_test("", "input_1.fpp input_2", Some("input"))
}

#[test]
fn locate_constant_1() {
    run_test("", "locate_constant_1", None)
}

#[test]
fn locate_constant_consistent() {
    run_test("", "locate_constant_consistent", None)
}

#[test]
fn locate_constant_include_i() {
    run_test(
        "-i locate_constant_include_i_output.out.txt",
        "locate_constant_include_1",
        Some("locate_constant_include_i"),
    )
}

#[test]
fn locate_constant_include_no_i() {
    run_test(
        "",
        "locate_constant_include_1",
        Some("locate_constant_include_no_i"),
    )
}

#[test]
fn locate_constant_inconsistent() {
    run_test("", "locate_constant_inconsistent", None)
}

#[test]
fn locate_constant_modules_1() {
    run_test("", "locate_constant_modules_1", None)
}

#[test]
fn locate_constant_modules_2() {
    run_test("", "locate_constant_modules_2", None)
}

#[test]
fn locate_constant_modules_3() {
    run_test("", "locate_constant_modules_3", None)
}

#[test]
fn locate_system_inconsistent() {
    run_test("", "locate_system_inconsistent", None)
}

#[test]
fn missing() {
    run_test("-m missing_output.out.txt", "missing", None)
}

#[test]
fn shadowed_absolute() {
    run_test("", "shadowed_absolute", None)
}

#[test]
fn shadowed_relative() {
    run_test("", "shadowed_relative", None)
}

#[test]
fn spec_async_input_port() {
    run_test("", "spec_async_input_port", None)
}

#[test]
fn spec_async_product_recv_port() {
    run_test("", "spec_async_product_recv_port", None)
}

#[test]
fn spec_command() {
    run_test("", "spec_command", None)
}

#[test]
fn spec_comp_instance() {
    run_test("", "spec_comp_instance", None)
}

#[test]
fn spec_connection_graph_direct() {
    run_test("", "spec_connection_graph_direct", None)
}

#[test]
fn spec_connection_graph_pattern() {
    run_test("", "spec_connection_graph_pattern", None)
}

#[test]
fn spec_container() {
    run_test("", "spec_container", None)
}

#[test]
fn spec_event() {
    run_test("", "spec_event", None)
}

#[test]
fn spec_import_interface() {
    run_test("", "spec_import_interface", None)
}

#[test]
fn spec_internal_port() {
    run_test("", "spec_internal_port", None)
}

#[test]
fn spec_param() {
    run_test("", "spec_param", None)
}

#[test]
fn spec_record() {
    run_test("", "spec_record", None)
}

#[test]
fn spec_state_machine_instance() {
    run_test("", "spec_state_machine_instance", None)
}

#[test]
fn spec_tlm_channel() {
    run_test("", "spec_tlm_channel", None)
}

#[test]
fn spec_tlm_packet() {
    run_test("", "spec_tlm_packet", None)
}

#[test]
fn spec_top_import() {
    run_test("", "spec_top_import", None)
}

#[test]
fn spec_top_port() {
    run_test("", "spec_top_port", None)
}

#[test]
fn special_ports() {
    run_test("", "special_ports", None)
}

#[test]
fn top_implements() {
    run_test("", "top_implements", None)
}

#[test]
fn string_no_size_implied_framework_defs() {
    run_test("", "string_no_size_implied_framework_defs", None)
}

#[test]
fn string_with_size_implied_framework_defs() {
    run_test("", "string_with_size_implied_framework_defs", None)
}

#[test]
fn template() {
    run_test("", "template", None)
}

#[test]
fn template_body_locate() {
    run_test("", "template_body_locate", None)
}

#[test]
fn template_body_locate_split() {
    run_test(
        "",
        "template_body_locate_split_a.fpp template_body_locate_split_b",
        Some("template_body_locate_split"),
    )
}

#[test]
fn template_cycle_a() {
    run_test("", "template_cycle_a", None)
}

#[test]
fn template_cycle_b() {
    run_test("", "template_cycle_b", None)
}

#[test]
fn template_dict_top() {
    run_test("", "template_dict_top", None)
}

#[test]
fn template_include() {
    run_test(
        "-i template_include_output.out.txt",
        "template_include",
        None,
    )
}

// The Scala test also writes the names of the generated files (-g, -u), which
// the Rust port does not compute; only the dependencies are compared
#[test]
fn template_incomplete() {
    run_test("", "template_incomplete", None)
}

#[test]
fn template_locate() {
    run_test("", "template_locate", None)
}

#[test]
fn template_type_arg_string() {
    run_test("", "template_type_arg_string", None)
}

#[test]
fn topology_implied_type() {
    run_test("", "topology_implied_type", None)
}

#[test]
fn transitive_instance() {
    run_test("", "transitive_instance", None)
}

// The tests below are Rust-only. Their reference outputs are the outputs of the
// Scala tool, except that a diagnostic is rendered the Rust way.

#[test]
fn nested_scope_prefix() {
    run_test("", "nested_scope_prefix", None)
}

#[test]
fn nested_module_prefix() {
    run_test("", "nested_module_prefix", None)
}

#[test]
fn uses_in_nested_scopes() {
    run_test("", "uses_in_nested_scopes", None)
}

#[test]
fn expr_dot_name_order() {
    run_test("", "expr_dot_name_order", None)
}

#[test]
fn qual_ident_name_order() {
    run_test("", "qual_ident_name_order", None)
}

#[test]
fn implements_scope() {
    run_test("", "implements_scope", None)
}

#[test]
fn implied_absolute() {
    run_test("", "implied_absolute", None)
}

#[test]
fn implied_enum_qualifier() {
    run_test("", "implied_enum_qualifier", None)
}

#[test]
fn implied_port_absolute() {
    run_test("", "implied_port_absolute", None)
}

#[test]
fn implied_port_absolute_interface() {
    run_test("", "implied_port_absolute_interface", None)
}

#[test]
fn implied_topology_absolute() {
    run_test("", "implied_topology_absolute", None)
}

#[test]
fn implied_topology_absolute_module() {
    run_test("", "implied_topology_absolute_module", None)
}

#[test]
fn sm_state_implied() {
    run_test("", "sm_state_implied", None)
}

#[test]
fn body_at_expansion_site() {
    run_test("", "body_at_expansion_site", None)
}

#[test]
fn template_args_nested() {
    run_test("", "template_args_nested", None)
}

#[test]
fn framework_template() {
    run_test(
        "-f framework_template_output.out.txt",
        "framework_template",
        None,
    )
}

#[test]
fn framework_template_expanded() {
    run_test(
        "-f framework_template_expanded_output.out.txt",
        "framework_template_expanded",
        None,
    )
}

#[test]
fn locate_constant_normalized() {
    run_test("", "locate_constant_normalized", None)
}

#[test]
fn inconsistent_then_use() {
    run_test("", "inconsistent_then_use", None)
}

#[test]
fn syntax_dep() {
    run_test("", "syntax_dep", None)
}

#[test]
fn missing_include_dep() {
    run_test(
        "-m missing_include_dep_output.out.txt",
        "missing_include_dep",
        None,
    )
}

#[test]
fn missing_include_input() {
    run_test("", "missing_include_input", None)
}

#[test]
fn include_in_state_machine() {
    run_test(
        "-i include_in_state_machine_output.out.txt",
        "include_in_state_machine",
        None,
    )
}

/// The test creates a directory symbolic link, which needs the Unix API
#[test]
#[cfg(unix)]
fn symlink_include() {
    let link = Path::new(env!("CARGO_TARGET_TMPDIR")).join("fpp_depend_link");
    let _ = fs::remove_file(&link);
    std::os::unix::fs::symlink(test_dir(), &link).expect("failed to create the link");
    run_test_in(
        &link,
        "-i symlink_include_output.out.txt",
        "symlink_include",
        None,
    )
}
