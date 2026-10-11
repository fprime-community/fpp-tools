use fpp_analysis::{
    Analysis, add_state_enums, check_semantics, resolve_includes, resolve_templates,
};
use fpp_core::{FileReader, SourceFile};
use fpp_fs::FsReader;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::{env, fs};

pub(crate) fn run_test(file_path: &str) {
    run_test_with(&[], file_path)
}

/// Run a test over several input files. The dependency paths are relative to the
/// tests directory and have no extension; the last file named is the one whose
/// `.ref.txt` is compared.
pub(crate) fn run_test_with(dep_paths: &[&str], file_path: &str) {
    run(dep_paths, file_path, false)
}

/// Run a test and also compare the unconnected port report that `fpp-check -u`
/// writes against `<file_path>-unconnected.ref.txt`.
///
/// The report lists, for each topology that has unconnected ports, the
/// qualified names of those ports. fpp-check writes the topologies in the
/// order of its topology map, which is not observable here, so the comparison
/// treats the report as a set of per-topology blocks.
pub(crate) fn run_test_unconnected(file_path: &str) {
    run(&[], file_path, true)
}

/// Write the unconnected port report the way `fpp-check -u` does: one block per
/// topology with unconnected ports, holding the topology's qualified name and
/// the sorted qualified names of its unconnected ports, followed by a blank
/// line. The blocks are in topology name order.
fn unconnected_port_report(a: &Analysis) -> String {
    let mut blocks: Vec<String> = a
        .topology_map
        .iter()
        .filter(|(_, t)| !t.unconnected_port_set.is_empty())
        .map(|(s, t)| {
            let mut ports: Vec<String> = t
                .unconnected_port_set
                .iter()
                .map(|pii| pii.qualified_name())
                .collect();
            ports.sort();
            let mut block = format!("Topology {}:\n", a.get_qualified_name(s));
            for port in ports {
                block.push_str(&format!("  {}\n", port));
            }
            block.push('\n');
            block
        })
        .collect();
    blocks.sort();
    blocks.concat()
}

/// Split an unconnected port report into its per-topology blocks, sorted
fn unconnected_report_blocks(report: &str) -> Vec<String> {
    let mut blocks: Vec<String> = report
        .split("\n\n")
        .map(|block| block.trim().to_string())
        .filter(|block| !block.is_empty())
        .collect();
    blocks.sort();
    blocks
}

fn run(dep_paths: &[&str], file_path: &str, unconnected: bool) {
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

    let mut report = None;
    let mut diagnostics_str = vec![];
    let mut ctx =
        fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics_str));

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
        if resolve_templates(&mut a, &mut asts).is_continue()
            && check_semantics(&mut a, asts.iter().collect()).is_continue()
            && unconnected
        {
            report = Some(unconnected_port_report(&a));
        }
    });

    let output = String::from_utf8(diagnostics_str)
        .expect("failed to convert error message to string")
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

    if unconnected {
        let report = report.expect("the analysis ran to completion");
        let mut report_file = path.clone();
        report_file.push(format!("{file_path}-unconnected"));
        report_file.set_extension("ref.txt");
        match env::var("FPP_UPDATE_REF") {
            Ok(_) => {
                // Keep a ref whose blocks match, since it may list them in
                // fpp-check's order
                let expected = fs::read_to_string(&report_file).ok();
                if expected.as_deref().map(unconnected_report_blocks)
                    != Some(unconnected_report_blocks(&report))
                {
                    fs::write(report_file, report).expect("failed to write the report ref")
                }
            }
            Err(_) => {
                let expected =
                    fs::read_to_string(&report_file).expect("failed to read the report ref");
                assert_eq!(
                    unconnected_report_blocks(&expected),
                    unconnected_report_blocks(&report)
                )
            }
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
