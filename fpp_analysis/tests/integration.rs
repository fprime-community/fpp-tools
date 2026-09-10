use fpp_analysis::{Analysis, add_state_enums, check_semantics, resolve_includes};
use fpp_core::{FileReader, SourceFile};
use fpp_fs::FsReader;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::{env, fs};

pub(crate) fn run_test(file_path: &str) {
    run_test_with(&[], file_path)
}

/// Run a test over several input files, as `fpp-check <deps> <file>` does. The
/// dependency paths are relative to the tests directory and have no extension;
/// the last file named is the one whose `.ref.txt` is compared.
pub(crate) fn run_test_with(dep_paths: &[&str], file_path: &str) {
    // Compute the path to the FPP input and .ref.txt output
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");

    let mut fpp_file = path.clone();
    fpp_file.push(file_path);
    fpp_file.set_extension("fpp");

    let mut ref_file = path.clone();
    ref_file.push(file_path);
    ref_file.set_extension("ref.txt");

    let dep_files: Vec<PathBuf> = dep_paths
        .iter()
        .map(|dep| {
            let mut dep_file = path.clone();
            dep_file.push(dep);
            dep_file.set_extension("fpp");
            dep_file
        })
        .collect();

    let file_reader = FsReader {};

    // Set up the compiler context to capture diagnostic messages into a buffer
    let mut diagnostics_str = vec![];
    let mut ctx =
        fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics_str));

    // Parse the input and run the semantic checker on the AST
    fpp_core::run(&mut ctx, || {
        let parse = |file: &PathBuf| {
            let source_file_path = file.to_str().unwrap();
            let src = match file_reader.read(source_file_path) {
                Ok(src) => SourceFile::new(source_file_path, src),
                Err(err) => panic!("failed to open {}: {}", source_file_path, err),
            };
            fpp_parser::parse(src, |p| p.trans_unit(), None)
        };

        let mut asts: Vec<_> = dep_files.iter().map(parse).collect();
        asts.push(parse(&fpp_file));
        let mut a = Analysis::new();
        for ast in &mut asts {
            let _ = resolve_includes(&mut a, FsReader {}, ast);
            add_state_enums(ast);
        }
        let _ = check_semantics(&mut a, asts.iter().collect());
    });

    let output = String::from_utf8(diagnostics_str)
        .expect("failed to convert error message to string")
        .replace(path.to_str().unwrap(), "[ local path prefix ]");

    // Validate the diagnostic messages against the reference file
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

mod cycles {
    mod test;
}

mod defs {
    mod test;
}

mod interface {
    mod test;
}

mod types {
    mod test;
}

mod port_matching {
    mod test;
}

mod record {
    mod test;
}

mod port_numbering {
    mod test;
}

mod array {
    mod test;
}

mod tlm_packets {
    mod test;
}

mod enums {
    mod test;
}

mod tlm_channel {
    mod test;
}

mod framework_defs {
    mod test;
}

mod expr {
    mod test;
}

mod component {
    mod test;
}

mod param {
    mod test;
}

mod container {
    mod test;
}

mod component_instance_def {
    mod test;
}

mod port_instance {
    mod test;
}

mod constant {
    mod test;
}

mod structs {
    mod test;
}

mod invalid_symbols {
    mod test;
}

mod redef {
    mod test;
}

mod state_machine_defs {
    mod test;
}

mod numeric_shift {
    mod test;
}

mod unconnected {
    mod test;
}

mod command {
    mod test;
}

mod port {
    mod test;
}

mod instance_spec {
    mod test;
}

mod connection_direct {
    mod test;
}

mod spec_init {
    mod test;
}

mod event {
    mod test;
}

mod spec_loc {
    mod test;
}

mod system {
    mod test;
}

mod top_import {
    mod test;
}

mod state_machine_instance {
    mod test;
}

mod connection_pattern {
    mod test;
}

mod internal_port {
    mod test;
}

mod top_ports {
    mod test;
}

#[cfg(feature = "disabled-tests")]
mod template {
    mod test;
}

mod state_machine {
    mod constants {
        mod test;
    }

    mod types {
        mod test;
    }

    mod initial_transitions {
        mod test;
    }

    mod transition_graph {
        mod test;
    }

    mod signal_uses {
        mod test;
    }

    mod redef {
        mod test;
    }

    mod typed_elements {
        mod test;
    }

    mod undef {
        mod test;
    }
}
