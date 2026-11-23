//! `fpp-diagram`: lower an FPP element into a diagram from the command line.
//!
//! Reads an FPP source file (or stdin), runs semantic analysis, and lowers the
//! named element to a diagram, emitting either Mermaid `stateDiagram-v2` source
//! (state machines) or sprotty `SModel` JSON to stdout. A nonzero exit code
//! signals an analysis error.
//!
//! The input is analyzed together with every file a locs file locates, so it may
//! use definitions from the rest of its project. The locs file is the one given
//! with `--locs` or, for an input file inside an F´ project, the `locs.fpp` in the
//! build cache of the project's default toolchain (found through the nearest
//! `settings.ini`).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::exit;

use clap::{Parser, ValueEnum};
use fpp_core::Spanned;
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

    /// A locs file locating the definitions the input uses. Every file it
    /// locates is analyzed with the input. Defaults to the `locs.fpp` of the F´
    /// project containing the input file.
    #[arg(long, conflicts_with = "no_locs")]
    locs: Option<PathBuf>,

    /// Analyze the input alone, without the locs file of its F´ project.
    #[arg(long)]
    no_locs: bool,

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

    let locs = locs_file(&args).map(|path| match std::fs::read_to_string(&path) {
        Ok(content) => (path, content),
        Err(e) => {
            eprintln!("failed to read {}: {e}", path.display());
            exit(2);
        }
    });

    let mut diagnostics = fpp_errors::ConsoleEmitter::color();
    let mut ctx = fpp_core::CompilerContext::new(&mut diagnostics);

    let output = fpp_core::run(&mut ctx, || {
        let src = fpp_core::SourceFile::new(&src_name, src_text);
        let mut tul = vec![fpp_parser::parse(src, |p| p.trans_unit(), None)];
        if let Some((path, content)) = locs {
            let input = args.file.as_deref().map(Path::new);
            tul.extend(located_units(&path, content, input));
        }

        let mut a = fpp_analysis::Analysis::new();
        let _ = fpp_analysis::CheckSemantics::tu_list(&mut a, fpp_fs::FsReader {}, &mut tul);

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

/// The locs file to analyze the input with, if any.
fn locs_file(args: &Args) -> Option<PathBuf> {
    if let Some(locs) = &args.locs {
        return Some(locs.clone());
    }
    if args.no_locs {
        return None;
    }
    // Stdin has no location to find a project from.
    let file = std::fs::canonicalize(args.file.as_ref()?).ok()?;
    let settings = fpp_fs::fprime::Settings::find(&file)?;
    let locs = settings.locs_file();
    if locs.is_file() {
        Some(locs)
    } else {
        eprintln!(
            "warning: no locs file at {}; analyzing {} alone (run `fprime-util generate` to create it)",
            locs.display(),
            file.display(),
        );
        None
    }
}

/// Parse every file the locs file at `path` locates, except `input`, which is
/// already part of the model. A file that locates several definitions is parsed
/// once.
///
/// Must be called inside a `fpp_core::run` scope. A located file that cannot be
/// read is reported at its location specifier.
fn located_units(path: &Path, content: String, input: Option<&Path>) -> Vec<fpp_ast::TransUnit> {
    let locs = fpp_core::SourceFile::new(&path.to_string_lossy(), content);
    let fpp_ast::TransUnit(members) = fpp_parser::parse(locs, |p| p.trans_unit(), None);

    let base = path.parent().unwrap_or(Path::new(""));
    let canonical = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let mut seen: rustc_hash::FxHashSet<PathBuf> = input.map(canonical).into_iter().collect();

    let mut units = Vec::new();
    for member in members {
        let fpp_ast::ModuleMember::SpecLoc(loc) = member else {
            continue;
        };
        let file = canonical(&base.join(&loc.file.data));
        if !seen.insert(file.clone()) {
            continue;
        }
        match std::fs::read_to_string(&file) {
            Ok(text) => {
                let src = fpp_core::SourceFile::new(&file.to_string_lossy(), text);
                units.push(fpp_parser::parse(src, |p| p.trans_unit(), None));
            }
            Err(e) => fpp_core::Diagnostic::new(
                loc.file.span(),
                fpp_core::Level::Error,
                "cannot read located file",
            )
            .annotation(format!("{}: {e}", file.display()))
            .emit(),
        }
    }
    units
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
