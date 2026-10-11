use fpp_lsp_parser::{TopEntryPoint, parse};
use pretty_assertions::assert_eq;
use std::path::{Path, PathBuf};
use std::{env, fs};

fn run_test_inner(fpp_file: &Path, ref_file: &Path, entry: TopEntryPoint) {
    let source_file_path = fpp_file.to_str().unwrap();
    let src = match fs::read_to_string(source_file_path) {
        Ok(src) => src,
        Err(err) => panic!("failed to open {}: {}", source_file_path, err),
    };

    let out = parse(&src, entry);
    let out_s = out.debug_dump();

    match env::var("FPP_UPDATE_REF") {
        Ok(_) => {
            // Update the ref file

            fs::write(ref_file, out_s).expect("failed to write ref.txt")
        }
        Err(_) => {
            // Read and compare against the ref file
            let ref_txt = fs::read_to_string(ref_file).expect("failed to read ref.txt");
            assert_eq!(ref_txt, out_s)
        }
    }
}

fn run_test(file_path: &str) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");

    let mut fpp_file = path.clone();
    fpp_file.push(file_path);
    fpp_file.set_extension("fpp");

    let mut ref_file = path.clone();
    ref_file.push(file_path);
    ref_file.set_extension("ref.txt");

    run_test_inner(&fpp_file, &ref_file, TopEntryPoint::Module);
}

fn run_test_from_fpp_parser(file_path: &str) {
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut fpp_file = cargo_dir.clone();
    fpp_file.push("../fpp_parser/tests");
    fpp_file.push(file_path);
    fpp_file.set_extension("fpp");

    let mut ref_file = cargo_dir.clone();
    ref_file.push("tests");
    ref_file.push(file_path);
    ref_file.set_extension("ref.txt");

    run_test_inner(&fpp_file, &ref_file, TopEntryPoint::Module);
}

/// Parse the include fragment `file_name` of fpp_parser/tests with the entry
/// point of the construct that includes it, and compare the tree with
/// `ref_name`.ref.txt.
fn run_fragment_test_from_fpp_parser(file_name: &str, entry: TopEntryPoint, ref_name: &str) {
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut fpp_file = cargo_dir.clone();
    fpp_file.push("../fpp_parser/tests");
    fpp_file.push(file_name);

    let mut ref_file = cargo_dir.clone();
    ref_file.push("tests");
    ref_file.push(ref_name);
    ref_file.set_extension("ref.txt");

    run_test_inner(&fpp_file, &ref_file, entry);
}

#[test]
fn simple() {
    run_test("simple")
}

#[test]
fn comments() {
    run_test_from_fpp_parser("comments")
}

#[test]
fn cycle_1() {
    run_test_from_fpp_parser("cycle-1")
}

#[test]
fn cycle_2() {
    run_test_from_fpp_parser("cycle-2")
}

#[test]
fn cycle_3() {
    run_test_from_fpp_parser("cycle-3")
}

#[test]
fn embedded_tab() {
    run_test_from_fpp_parser("embedded-tab")
}

#[test]
fn empty() {
    run_test_from_fpp_parser("empty")
}

#[test]
fn escaped_strings() {
    run_test_from_fpp_parser("escaped-strings")
}

#[test]
fn illegal_character() {
    run_test_from_fpp_parser("illegal-character")
}

#[test]
fn include_component() {
    run_test_from_fpp_parser("include-component")
}

#[test]
fn include_constant_1() {
    run_test_from_fpp_parser("include-constant-1")
}

#[test]
fn include_missing_file() {
    run_test_from_fpp_parser("include-missing-file")
}

#[test]
fn include_module() {
    run_test_from_fpp_parser("include-module")
}

#[test]
fn include_parse_error() {
    run_test_from_fpp_parser("include-parse-error")
}

#[test]
fn include_subdir() {
    run_test_from_fpp_parser("include-subdir")
}

#[test]
fn include_topology() {
    run_test_from_fpp_parser("include-topology")
}

// The fragments that the include tests include, each parsed as the members
// of the construct that includes it

#[test]
fn fragment_constant_in_component() {
    run_fragment_test_from_fpp_parser(
        "constant.fppi",
        TopEntryPoint::Component,
        "fragment-constant-component",
    )
}

#[test]
fn fragment_constant_in_module() {
    run_fragment_test_from_fpp_parser(
        "constant.fppi",
        TopEntryPoint::Module,
        "fragment-constant-module",
    )
}

#[test]
fn fragment_instance() {
    run_fragment_test_from_fpp_parser(
        "instance.fppi",
        TopEntryPoint::Topology,
        "fragment-instance",
    )
}

#[test]
fn fragment_packet() {
    run_fragment_test_from_fpp_parser(
        "packet.fppi",
        TopEntryPoint::TlmPacketSet,
        "fragment-packet",
    )
}

#[test]
fn fragment_channel() {
    run_fragment_test_from_fpp_parser("channel.fppi", TopEntryPoint::TlmPacket, "fragment-channel")
}

#[test]
fn parse_error() {
    run_test_from_fpp_parser("parse-error")
}

#[test]
fn expr_shift_sizeof() {
    run_test_from_fpp_parser("expr-shift-sizeof")
}

#[test]
fn state_machine_defs() {
    run_test_from_fpp_parser("state-machine-defs")
}

#[test]
fn state_machine() {
    run_test_from_fpp_parser("state-machine")
}

#[test]
fn syntax() {
    run_test_from_fpp_parser("syntax")
}

#[test]
fn syntax_kwd_names() {
    run_test_from_fpp_parser("syntax-kwd-names")
}

#[test]
fn topology_ports() {
    run_test_from_fpp_parser("topology-ports")
}

#[test]
fn topology() {
    run_test_from_fpp_parser("topology")
}

#[test]
fn guarded_command_nested_array() {
    run_test("guarded-command-nested-array")
}

#[test]
fn template() {
    run_test_from_fpp_parser("template")
}
