//! `fpp-diagram`: lower an FPP element into a diagram from the command line.
//!
//! Reads an FPP source file (or stdin), runs semantic analysis, and lowers the
//! named element to a diagram, emitting either Mermaid `stateDiagram-v2` source
//! (state machines) or sprotty `SModel` JSON to stdout. A nonzero exit code
//! signals an analysis error.

use std::io::Read;
use std::process::exit;

use clap::{Parser, ValueEnum};
use fpp_diagram::{DiagramKind, TransitionActionMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ModeArg {
    /// Edge labels show only each transition's own `do { }` actions; entry/exit
    /// actions are shown inside the state.
    Uml,
    /// Edge labels show the full flattened action sequence that runs on the
    /// transition (exit + do + entry, including the target leaf's entry).
    Flattened,
}

impl From<ModeArg> for TransitionActionMode {
    fn from(m: ModeArg) -> Self {
        match m {
            ModeArg::Uml => TransitionActionMode::Uml,
            ModeArg::Flattened => TransitionActionMode::Flattened,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum FormatArg {
    /// Mermaid `stateDiagram-v2` source text (state machines only).
    Mermaid,
    /// sprotty `SModel` JSON.
    Sprotty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum KindArg {
    Component,
    Topology,
    ConnectionGroup,
    StateMachine,
}

impl From<KindArg> for DiagramKind {
    fn from(k: KindArg) -> Self {
        match k {
            KindArg::Component => DiagramKind::Component,
            KindArg::Topology => DiagramKind::Topology,
            KindArg::ConnectionGroup => DiagramKind::ConnectionGroup,
            KindArg::StateMachine => DiagramKind::StateMachine,
        }
    }
}

/// Lower an FPP element into a diagram.
#[derive(Parser, Debug)]
#[command(name = "fpp-diagram", version, about)]
struct Args {
    /// The fully qualified name of the element to diagram. For a connection
    /// group this is `<topology>.<group>`.
    #[arg(long)]
    name: String,

    /// The kind of element to diagram.
    #[arg(long, value_enum, default_value_t = KindArg::StateMachine)]
    kind: KindArg,

    /// How state machine transition actions are presented (state machines only).
    #[arg(long, value_enum, default_value_t = ModeArg::Uml)]
    mode: ModeArg,

    /// Output format. Mermaid is valid for state machines only; other kinds
    /// always emit sprotty JSON.
    #[arg(long, value_enum, default_value_t = FormatArg::Mermaid)]
    format: FormatArg,

    /// Prune ports not referenced by any connection (topology diagrams only).
    #[arg(long)]
    hide_unused_ports: bool,

    /// The FPP source file to read. Reads from stdin when omitted.
    file: Option<String>,
}

fn main() {
    let args = Args::parse();

    let src_text = match &args.file {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("failed to read {path}: {e}");
                exit(2);
            }
        },
        None => {
            let mut s = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut s) {
                eprintln!("failed to read stdin: {e}");
                exit(2);
            }
            s
        }
    };
    let src_name = args.file.clone().unwrap_or_else(|| "<stdin>".to_string());

    let mut diagnostics = fpp_errors::ConsoleEmitter::color();
    let mut ctx = fpp_core::CompilerContext::new(&mut diagnostics);

    let output = fpp_core::run(&mut ctx, || {
        let src = fpp_core::SourceFile::new(&src_name, src_text);
        let mut ast = fpp_parser::parse(src, |p| p.trans_unit(), None);

        let mut a = fpp_analysis::Analysis::new();
        let _ = fpp_analysis::resolve_includes(&mut a, fpp_fs::FsReader {}, &mut ast);
        fpp_analysis::add_state_enums(&mut ast);
        if fpp_analysis::resolve_templates(&mut a, std::slice::from_mut(&mut ast)).is_continue() {
            let _ = fpp_analysis::check_semantics(&mut a, vec![&ast]);
        }

        lower(&a, &args)
    });

    if diagnostics.has_errors() {
        exit(1);
    }

    match output {
        Ok(text) => println!("{text}"),
        Err(e) => {
            eprintln!("{e}");
            exit(1);
        }
    }
}

/// Lower the requested element to its output string.
fn lower(a: &fpp_analysis::Analysis, args: &Args) -> Result<String, fpp_diagram::LowerError> {
    let kind: DiagramKind = args.kind.into();
    let mode: TransitionActionMode = args.mode.into();

    match args.format {
        FormatArg::Mermaid if kind == DiagramKind::StateMachine => {
            fpp_diagram::lower_state_machine_to_mermaid(a, &args.name, mode)
        }
        FormatArg::Mermaid => {
            // Mermaid is only implemented for state machines; other kinds fall
            // back to sprotty JSON.
            let json =
                fpp_diagram::lower_to_smodel(a, kind, &args.name, args.hide_unused_ports, mode)?;
            Ok(json.to_string())
        }
        FormatArg::Sprotty => {
            let json =
                fpp_diagram::lower_to_smodel(a, kind, &args.name, args.hide_unused_ports, mode)?;
            Ok(json.to_string())
        }
    }
}
