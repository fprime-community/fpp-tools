use fpp_format::{FormatOptions, Formatter};
use fpp_lsp_parser::{TopEntryPoint, parse};
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::{env, fs};

fn format_src(
    text: &str,
    entry: TopEntryPoint,
    options: FormatOptions,
) -> Result<String, fpp_format::FormatError> {
    let parse = parse(text, entry);
    let errors = parse.errors();
    if !errors.is_empty() {
        return Err(fpp_format::FormatError::ParseError(errors));
    }
    let root = parse.syntax_node();
    Ok(Formatter::new(options).format(&root))
}

fn dup(entry: &TopEntryPoint) -> TopEntryPoint {
    match entry {
        TopEntryPoint::Module => TopEntryPoint::Module,
        TopEntryPoint::Component => TopEntryPoint::Component,
        TopEntryPoint::Topology => TopEntryPoint::Topology,
        TopEntryPoint::TlmPacket => TopEntryPoint::TlmPacket,
        TopEntryPoint::TlmPacketSet => TopEntryPoint::TlmPacketSet,
    }
}

fn run_test(file_name: &str, entry: TopEntryPoint) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");

    let mut input_file = path.clone();
    input_file.push(file_name);
    input_file.set_extension("fpp");

    let mut ref_file = path.clone();
    ref_file.push(file_name);
    ref_file.set_extension("ref.fpp");

    let src = match fs::read_to_string(&input_file) {
        Ok(src) => src,
        Err(err) => {
            eprintln!("Skipping {}: {}", input_file.display(), err);
            return;
        }
    };

    let formatted = match format_src(&src, dup(&entry), FormatOptions::default()) {
        Ok(f) => f,
        Err(e) => panic!("{}: formatting failed: {:?}", file_name, e),
    };

    match env::var("FPP_UPDATE_REF") {
        Ok(_) => fs::write(&ref_file, &formatted).expect("failed to write ref.fpp"),
        Err(_) => {
            let expected = fs::read_to_string(&ref_file)
                .unwrap_or_else(|e| panic!("failed to read {}: {}", ref_file.display(), e));
            assert_eq!(
                expected, formatted,
                "{}: output differs from golden",
                file_name
            );
        }
    }

    let parse_result = parse(&formatted, dup(&entry));
    if !parse_result.errors().is_empty() {
        panic!(
            "{}: output has parse errors: {:?}\nOutput:\n{}",
            file_name,
            parse_result.errors(),
            formatted
        );
    }

    let formatted2 = format_src(&formatted, entry, FormatOptions::default())
        .expect("second format should succeed");
    if formatted != formatted2 {
        panic!(
            "{}: not idempotent\nFirst:\n{}\nSecond:\n{}",
            file_name, formatted, formatted2
        );
    }

    eprintln!("{}: ok (reparse + idempotent)", file_name);
}

macro_rules! fmt_test {
    ($name:ident, $file:expr) => {
        #[test]
        fn $name() {
            run_test($file, TopEntryPoint::Module);
        }
    };
}

fmt_test!(simple_module, "simple-module");
fmt_test!(nested_module, "nested-module");
fmt_test!(comments, "comments");
fmt_test!(multiple_definitions, "multiple-definitions");
fmt_test!(binary_expressions, "binary-expressions");
fmt_test!(annotations, "annotations");
fmt_test!(array_struct, "array-struct");
fmt_test!(component, "component");
fmt_test!(port, "port");
fmt_test!(topology, "topology");
fmt_test!(state_machine, "state-machine");
fmt_test!(state_machine_defs, "state-machine-defs");
fmt_test!(instances_locate, "instances-locate");
fmt_test!(interface_include, "interface-include");
fmt_test!(expressions, "expressions");
fmt_test!(expr_shift_sizeof, "expr-shift-sizeof");
fmt_test!(type_defs, "type-defs");
fmt_test!(topology_extra, "topology-extra");
fmt_test!(component_extra, "component-extra");
fmt_test!(multiline_string_align, "multiline-string-align");
fmt_test!(constant_align, "constant-align");
fmt_test!(enum_annotation_align, "enum-annotation-align");
fmt_test!(param_annotation, "param-annotation");
fmt_test!(blank_lines, "blank-lines");

#[test]
fn component_inner() {
    run_test("component-inner", TopEntryPoint::Component);
}
