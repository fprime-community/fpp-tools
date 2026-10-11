//! The tests of `fpp-syntax -e`, which expands the syntax of a model before
//! printing its syntax tree.
//!
//! The expansion is [`ExpandSyntaxAndEnterSymbols`]: it resolves the include
//! specifiers, adds the state enums, enters the symbols and expands the
//! templates, so that each template expansion specifier holds a copy of the
//! template body. The fpp_parser tests, which cover the other `fpp-syntax`
//! tests, stop before the expansion.

use fpp_analysis::{Analysis, ExpandSyntaxAndEnterSymbols};
use fpp_core::{FileReader, SourceFile};
use fpp_fs::FsReader;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::{env, fs};

/// Parse `fpp_syntax_expand/<file_path>.fpp`, expand its syntax, and compare
/// the expanded syntax tree, or the diagnostics if there are any, with
/// `fpp_syntax_expand/<file_path>.ref.txt`
fn run_test(file_path: &str) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("fpp_syntax_expand");

    let mut fpp_file = path.clone();
    fpp_file.push(file_path);
    fpp_file.set_extension("fpp");

    let mut ref_file = path.clone();
    ref_file.push(file_path);
    ref_file.set_extension("ref.txt");

    let mut diagnostics = vec![];
    let mut ctx = fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
    let ast = fpp_core::run(&mut ctx, || {
        let source_file_path = fpp_file.to_str().unwrap();
        let reader = FsReader {};
        let src = match reader.read(source_file_path) {
            Ok(src) => SourceFile::new(source_file_path, src),
            Err(err) => panic!("failed to open {}: {}", source_file_path, err),
        };
        let mut tul = vec![fpp_parser::parse(src, |p| p.trans_unit(), None)];
        let mut a = Analysis::new();
        let _ = ExpandSyntaxAndEnterSymbols::tu_list(&mut a, reader, &mut tul);
        format!("{:#?}", tul[0])
    });

    let output = if diagnostics.is_empty() {
        ast
    } else {
        String::from_utf8(diagnostics).expect("diagnostics are UTF-8")
    }
    .replace(path.to_str().unwrap(), "[ local path prefix ]");

    match env::var("FPP_UPDATE_REF") {
        Ok(_) => fs::write(ref_file, output).expect("failed to write ref.txt"),
        Err(_) => {
            let ref_txt = fs::read_to_string(ref_file).expect("failed to read ref.txt");
            assert_eq!(ref_txt, output)
        }
    }
}

#[test]
fn template() {
    run_test("template")
}
