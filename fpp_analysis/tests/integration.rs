use fpp_analysis::{Analysis, CheckSemantics};
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
/// qualified names of those ports.
pub(crate) fn run_test_unconnected(file_path: &str) {
    run(&[], file_path, true)
}

/// Write the unconnected port report the way `fpp-check -u` does: one block per
/// topology with unconnected ports, holding the topology's qualified name and
/// the sorted qualified names of its unconnected ports, followed by a blank
/// line. The blocks are in the order of the topology map, which is the order
/// the topologies were resolved.
fn unconnected_port_report(a: &Analysis) -> String {
    let mut report = String::new();
    for (s, t) in a
        .topology_map
        .iter()
        .filter(|(_, t)| !t.unconnected_port_set.is_empty())
    {
        let mut ports: Vec<String> = t
            .unconnected_port_set
            .iter()
            .map(|pii| pii.qualified_name())
            .collect();
        ports.sort();
        report.push_str(&format!("Topology {}:\n", a.get_qualified_name(s)));
        for port in ports {
            report.push_str(&format!("  {}\n", port));
        }
        report.push('\n');
    }
    report
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
        if CheckSemantics::tu_list(&mut a, FsReader {}, &mut asts).is_continue() && unconnected {
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
            Ok(_) => fs::write(report_file, report).expect("failed to write the report ref"),
            Err(_) => {
                let expected =
                    fs::read_to_string(&report_file).expect("failed to read the report ref");
                assert_eq!(expected, report)
            }
        }
    }
}

mod array {
    mod test;
}

mod command {
    mod test;
}

mod component {
    mod test;
}

mod component_instance_def {
    mod test;
}

mod connection_direct {
    mod test;
}

mod connection_pattern {
    mod test;
}

mod constant {
    mod test;
}

mod container {
    mod test;
}

mod cycle {
    mod test;
}

mod defs {
    mod test;
}

mod r#enum {
    mod test;
}

mod event {
    mod test;
}

mod expr {
    mod test;
}

mod framework_defs {
    mod test;
}

mod instance_spec {
    mod test;
}

mod interface {
    mod test;
}

mod internal_port {
    mod test;
}

mod invalid_symbols {
    mod test;
}

mod numeric_shift {
    mod test;
}

mod param {
    mod test;
}

mod port {
    mod test;
}

mod port_instance {
    mod test;
}

mod port_matching {
    mod test;
}

mod port_numbering {
    mod test;
}

mod record {
    mod test;
}

mod redef {
    mod test;
}

mod spec_init {
    mod test;
}

mod spec_loc {
    mod test;
}

mod state_machine {
    mod test;

    mod constants {
        mod test;
    }

    mod initial_transitions {
        mod test;
    }

    mod redef {
        mod test;
    }

    mod signal_uses {
        mod test;
    }

    mod transition_graph {
        mod test;
    }

    mod typed_elements {
        mod test;
    }

    mod types {
        mod test;
    }

    mod undef {
        mod test;
    }
}

mod state_machine_instance {
    mod test;
}

mod r#struct {
    mod test;
}

mod system {
    mod test;
}

mod template {
    mod test;
}

mod tlm_channel {
    mod test;
}

mod tlm_packets {
    mod test;
}

mod top_import {
    mod test;
}

mod top_ports {
    mod test;
}

mod r#type {
    mod test;
}

mod unconnected {
    mod test;
}

// Rust-only test directories (no Scala counterpart)

mod state_machine_defs {
    mod test;
}
