use fpp_ast::MutVisitor;
use fpp_core::{FileReader, SourceFile};
use fpp_fs::FsReader;
use fpp_parser::{ResolveIncludes, parse};
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::{env, fs};

fn run_test(file_path: &str) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");

    let mut fpp_file = path.clone();
    fpp_file.push(file_path);
    fpp_file.set_extension("fpp");

    let mut ref_file = path.clone();
    ref_file.push(file_path);
    ref_file.set_extension("ref.txt");

    let file_reader = FsReader {};

    let mut diagnostics_str = vec![];
    let mut ctx =
        fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics_str));

    let ast: String = fpp_core::run(&mut ctx, || {
        let source_file_path = fpp_file.to_str().unwrap();
        let src = match file_reader.read(source_file_path) {
            Ok(src) => SourceFile::new(source_file_path, src),
            Err(err) => panic!("failed to open {}: {}", source_file_path, err),
        };

        // Parse the source
        let mut ast = parse(src, |p| p.trans_unit(), None);
        let mut source_files = Default::default();
        let _ = ResolveIncludes::new(file_reader).visit_trans_unit(&mut source_files, &mut ast);
        format!("{:#?}", ast)
    });

    let output = if diagnostics_str.is_empty() {
        ast
    } else {
        String::from_utf8(diagnostics_str).expect("failed to convert error message to string")
    }
    .replace(path.to_str().unwrap(), "[ local path prefix ]");

    match env::var("FPP_UPDATE_REF") {
        Ok(_) => {
            // Update the ref file
            fs::write(ref_file, output).expect("failed to write ref.txt")
        }
        Err(_) => {
            // Read and compare against the ref file
            let ref_txt = fs::read_to_string(ref_file).expect("failed to read ref.txt");
            assert_eq!(ref_txt, output)
        }
    }
}

#[test]
fn comments() {
    run_test("comments")
}

#[test]
fn crlf() {
    run_test("crlf")
}

#[test]
fn cycle_1() {
    run_test("cycle-1")
}

#[test]
fn cycle_2() {
    run_test("cycle-2")
}

#[test]
fn cycle_3() {
    run_test("cycle-3")
}

#[test]
fn embedded_tab() {
    run_test("embedded-tab")
}

#[test]
fn empty() {
    run_test("empty")
}

#[test]
fn escaped_strings() {
    run_test("escaped-strings")
}

#[test]
fn illegal_character() {
    run_test("illegal-character")
}

#[test]
fn include_component() {
    run_test("include-component")
}

#[test]
fn include_constant_1() {
    run_test("include-constant-1")
}

#[test]
fn include_missing_file() {
    run_test("include-missing-file")
}

#[test]
fn include_module() {
    run_test("include-module")
}

#[test]
fn include_parse_error() {
    run_test("include-parse-error")
}

#[test]
fn include_subdir() {
    run_test("include-subdir")
}

#[test]
fn include_topology() {
    run_test("include-topology")
}

#[test]
fn numeric_shift() {
    run_test("numeric-shift")
}

#[test]
fn parse_error() {
    run_test("parse-error")
}

#[test]
fn expr_shift_sizeof() {
    run_test("expr-shift-sizeof")
}

#[test]
fn state_machine_defs() {
    run_test("state-machine-defs")
}

#[test]
fn state_machine() {
    run_test("state-machine")
}

#[test]
fn syntax() {
    run_test("syntax")
}

#[test]
fn syntax_kwd_names() {
    run_test("syntax-kwd-names")
}

#[test]
fn system() {
    run_test("system")
}

#[test]
fn topology_ports() {
    run_test("topology-ports")
}

#[test]
fn topology() {
    run_test("topology")
}

#[test]
fn template() {
    run_test("template")
}

// Rust-only: the template forms that `template` leaves out
#[test]
fn template_forms() {
    run_test("template-forms")
}

#[test]
fn template_error() {
    run_test("template-error")
}

#[test]
fn template_error_def() {
    run_test("template-error-def")
}

#[test]
fn template_error_body() {
    run_test("template-error-body")
}

#[test]
fn template_error_locate_name() {
    run_test("template-error-locate-name")
}

#[test]
fn template_error_locate_file() {
    run_test("template-error-locate-file")
}
