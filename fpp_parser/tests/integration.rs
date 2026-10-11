use fpp_ast::MutVisitor;
use fpp_core::{FileReader, SourceFile};
use fpp_fs::FsReader;
use fpp_parser::{ResolveIncludes, parse};
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::{env, fs};

/// Parse `file_path`.fpp, resolve its includes, and compare the result with
/// `file_path`.ref.txt
fn run_test(file_path: &str) {
    run_test_files(&[file_path], file_path)
}

/// Parse each of the files `file_paths` (without the `.fpp`), resolve their
/// includes, and compare the result with `ref_path`.ref.txt
fn run_test_files(file_paths: &[&str], ref_path: &str) {
    run_test_with(file_paths, ref_path, true)
}

/// Parse each of the files `file_paths` (without the `.fpp`), resolve their
/// includes if `resolve_includes` is set, and compare the result with
/// `ref_path`.ref.txt
fn run_test_with(file_paths: &[&str], ref_path: &str, resolve_includes: bool) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");

    let mut ref_file = path.clone();
    ref_file.push(ref_path);
    ref_file.set_extension("ref.txt");

    let file_reader = FsReader {};

    let mut diagnostics_str = vec![];
    let mut ctx =
        fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics_str));

    let ast: String = fpp_core::run(&mut ctx, || {
        let mut out = String::new();
        for file_path in file_paths {
            let mut fpp_file = path.clone();
            fpp_file.push(file_path);
            fpp_file.set_extension("fpp");
            let source_file_path = fpp_file.to_str().unwrap();
            let src = match file_reader.read(source_file_path) {
                Ok(src) => SourceFile::new(source_file_path, src),
                Err(err) => panic!("failed to open {}: {}", source_file_path, err),
            };

            // Parse the source
            let mut ast = parse(src, |p| p.trans_unit(), None);
            if resolve_includes {
                let mut source_files = Default::default();
                let _ =
                    ResolveIncludes::new(FsReader {}).visit_trans_unit(&mut source_files, &mut ast);
            }
            out.push_str(&format!("{:#?}", ast));
        }
        out
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
fn embedded_tab() {
    run_test("embedded-tab")
}

#[test]
fn empty() {
    run_test("empty")
}

#[test]
fn escaped_include_path() {
    run_test("escaped-include-path")
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
fn include_cycle_1() {
    run_test_files(&["cycle-1"], "include-cycle-1")
}

#[test]
fn include_cycle_2() {
    run_test_files(&["cycle-2"], "include-cycle-2")
}

// Rust-only: the cycle entered at its third file
#[test]
fn include_cycle_3() {
    run_test_files(&["cycle-3"], "include-cycle-3")
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
fn include_packet_comment() {
    run_test("include-packet-comment")
}

#[test]
fn include_parent_dir() {
    run_test("subdir/include-parent-dir")
}

#[test]
fn include_parse_error() {
    run_test("include-parse-error")
}

#[test]
fn include_state_machine() {
    run_test("include-state-machine")
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
fn implements_newline() {
    run_test("implements-newline")
}

#[test]
fn numeric_shift() {
    run_test("numeric-shift")
}

#[test]
fn parse_error() {
    run_test("parse-error")
}

// Rust-only: the errors reported after the first one
#[test]
fn parse_error_recovery() {
    run_test("parse-error-recovery")
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
fn syntax_ast() {
    run_test_with(&["syntax"], "syntax-ast", false)
}

#[test]
fn syntax_include_ast() {
    run_test_files(&["syntax"], "syntax-include-ast")
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
fn two_input_files() {
    run_test_files(&["include-module", "include-component"], "two-input-files")
}

#[test]
fn dictionary_specifier() {
    run_test("dictionary-specifier")
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

// Rust-only: the element sequences, annotations and commit points of the grammar
#[test]
fn stray_brace() {
    run_test("stray-brace")
}

#[test]
fn dangling_annotation() {
    run_test("dangling-annotation")
}

#[test]
fn element_sequence() {
    run_test("element-sequence")
}

#[test]
fn implements_newline_error() {
    run_test("implements-newline-error")
}

#[test]
fn missing_separator() {
    run_test("missing-separator")
}

#[test]
fn post_annotation_element() {
    run_test("post-annotation-element")
}

#[test]
fn locate_dictionary_error() {
    run_test("locate-dictionary-error")
}

#[test]
fn locate_dictionary_eof() {
    run_test("locate-dictionary-eof")
}

#[test]
fn locate_state_eof() {
    run_test("locate-state-eof")
}

#[test]
fn type_name_eof() {
    run_test("type-name-eof")
}

#[test]
fn transition_eof() {
    run_test("transition-eof")
}

#[test]
fn state_machine_dictionary() {
    run_test("state-machine-dictionary")
}

#[test]
fn commit_points() {
    run_test("commit-points")
}

#[test]
fn uncommitted_member() {
    run_test("uncommitted-member")
}

#[test]
fn eof_in_block() {
    run_test("eof-in-block")
}

#[test]
fn interface_member_error() {
    run_test("interface-member-error")
}

// Rust-only: include resolution
#[test]
fn include_stray_brace() {
    run_test("include-stray-brace")
}

#[test]
fn include_normalized_path() {
    run_test("include-normalized-path")
}

#[test]
fn include_self() {
    run_test("include-self")
}

#[test]
fn include_state_machine_missing_file() {
    run_test("include-state-machine-missing-file")
}

#[test]
fn include_packet_leading_comment() {
    run_test("include-packet-leading-comment")
}
