//! Tests for the implied uses of a deployment topology. They exist only when
//! the analysis generates dictionaries, so these tests turn that on, as a
//! dictionary generator does.

use fpp_analysis::{Analysis, add_state_enums, check_semantics, resolve_templates};
use fpp_core::{FileReader, SourceFile};
use fpp_fs::FsReader;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::{env, fs};

fn run_test(name: &str) {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("tests");
    let fpp_file = dir
        .join("dictionary_implied_uses")
        .join(format!("{name}.fpp"));
    let ref_file = fpp_file.with_extension("ref.txt");

    let mut diagnostics = vec![];
    let mut ctx = fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
    fpp_core::run(&mut ctx, || {
        let path = fpp_file.to_str().unwrap();
        let src = FsReader {}
            .read(path)
            .unwrap_or_else(|err| panic!("failed to open {path}: {err}"));
        let mut ast = fpp_parser::parse(SourceFile::new(path, src), |p| p.trans_unit(), None);
        add_state_enums(&mut ast);
        let mut a = Analysis::new();
        a.dictionary_generation = true;
        if resolve_templates(&mut a, std::slice::from_mut(&mut ast)).is_continue() {
            let _ = check_semantics(&mut a, vec![&ast]);
        }
    });

    let output = String::from_utf8(diagnostics)
        .expect("failed to convert error message to string")
        .replace(dir.to_str().unwrap(), "[ local path prefix ]");
    match env::var("FPP_UPDATE_REF") {
        Ok(_) => fs::write(ref_file, output).expect("failed to write ref.txt"),
        Err(_) => {
            let ref_txt = fs::read_to_string(ref_file).expect("failed to read ref.txt");
            assert_eq!(ref_txt, output)
        }
    }
}

/// The implied use `Fw.DpCfg.CONTAINER_USER_DATA_SIZE` names a member of the
/// constant `Fw.DpCfg`
#[test]
fn member_of_constant() {
    run_test("member_of_constant")
}

/// The implied use `Fw.DpCfg.CONTAINER_USER_DATA_SIZE` names no definition
#[test]
fn missing_user_data_size() {
    run_test("missing_user_data_size")
}
