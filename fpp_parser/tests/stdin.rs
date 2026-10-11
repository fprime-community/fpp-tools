//! The fpp-syntax `syntax_stdin` test: a model read from standard input
//! includes files relative to the current directory.
//!
//! The test changes the current directory of the process, so it is the only
//! test in this file.

use fpp_ast::MutVisitor;
use fpp_core::SourceFile;
use fpp_fs::FsReader;
use fpp_parser::{ResolveIncludes, parse};
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::{env, fs};

#[test]
fn syntax_stdin() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    env::set_current_dir(&path).expect("failed to enter the test directory");

    let content = fs::read_to_string("syntax.fpp").expect("failed to read syntax.fpp");
    let mut diagnostics_str = vec![];
    let mut ctx =
        fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics_str));
    let ast = fpp_core::run(&mut ctx, || {
        let src = SourceFile::new("<stdin>", content);
        let mut ast = parse(src, |p| p.trans_unit(), None);
        let mut source_files = Default::default();
        let _ = ResolveIncludes::new(FsReader {}).visit_trans_unit(&mut source_files, &mut ast);
        format!("{:#?}", ast)
    });
    drop(ctx);

    let output = if diagnostics_str.is_empty() {
        ast
    } else {
        String::from_utf8(diagnostics_str).expect("failed to convert error message to string")
    }
    .replace(path.to_str().unwrap(), "[ local path prefix ]");

    let ref_file = path.join("syntax-stdin.ref.txt");
    match env::var("FPP_UPDATE_REF") {
        Ok(_) => fs::write(ref_file, output).expect("failed to write ref.txt"),
        Err(_) => {
            let ref_txt = fs::read_to_string(ref_file).expect("failed to read ref.txt");
            assert_eq!(ref_txt, output)
        }
    }
}
