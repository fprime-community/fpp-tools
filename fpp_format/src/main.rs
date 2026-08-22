use clap::Parser;
use fpp_format::{FormatOptions, PartialConfig, format_file_recursive, format_text, load_config};
use fpp_lsp_parser::TopEntryPoint;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process;

/// A formatter (pretty-printer) for the F Prime Prime (FPP) modeling language.
///
/// By default only the named files are formatted. Pass --recursive-includes to
/// additionally follow `include` specifiers and format each `.fppi` fragment
/// with the entrypoint derived from its include context.
///
/// The indentation width and maximum line length are read from the nearest
/// `.fpp-format` file (searched upward from each file's directory), falling
/// back to built-in defaults (4-space indent, 80-column width). The --indent
/// and --line-length flags override whatever the file specifies.
#[derive(Parser, Debug)]
#[command(version, author, about, long_about)]
struct Args {
    /// FPP files to format. If none are given, reads from stdin.
    #[arg(value_name = "FILES")]
    files: Vec<PathBuf>,

    /// Check if files are formatted without writing; exit 1 if any is not.
    #[arg(long)]
    check: bool,

    /// Read from stdin and write to stdout (default when no files are given).
    #[arg(long)]
    stdin: bool,

    /// Also follow `include` specifiers and format reachable `.fppi` fragments.
    #[arg(long)]
    recursive_includes: bool,

    /// Number of spaces per indentation level. Overrides any `.fpp-format` file.
    #[arg(long, value_name = "N")]
    indent: Option<usize>,

    /// Maximum line width before specs explode their clauses and group-style
    /// member lists break onto multiple lines. Overrides any `.fpp-format` file.
    #[arg(long, value_name = "N")]
    line_length: Option<usize>,

    /// Parser entrypoint / grammar rule for the input. One of: module (default),
    /// component, topology, tlm-packet, tlm-packet-set. Needed when formatting a
    /// bare `.fppi` fragment directly.
    #[arg(long, value_name = "RULE", value_parser = parse_entry)]
    entry: Option<TopEntryPoint>,
}

impl Args {
    /// The formatting overrides supplied explicitly on the command line. These
    /// take precedence over any discovered `.fpp-format` file.
    fn cli_overrides(&self) -> PartialConfig {
        PartialConfig {
            indent_width: self.indent,
            max_line_width: self.line_length,
        }
    }

    /// Resolve the [`FormatOptions`] governing files in `dir` by layering the
    /// CLI overrides on top of the nearest `.fpp-format` file (and defaults).
    ///
    /// A malformed `.fpp-format` file aborts the run — silently ignoring it
    /// would format with the wrong profile and, under `--check`, report false
    /// failures.
    fn resolve_options(&self, dir: &Path) -> FormatOptions {
        let file_config = load_config(dir).unwrap_or_else(|err| {
            eprintln!("Error reading {}: {}", fpp_format::CONFIG_FILE_NAME, err);
            process::exit(1);
        });
        file_config.merge(self.cli_overrides()).into_options()
    }
}

/// Parse a `--entry` value into a [`TopEntryPoint`].
fn parse_entry(s: &str) -> Result<TopEntryPoint, String> {
    TopEntryPoint::from_name(s).ok_or_else(|| format!("unknown entrypoint: {s}"))
}

fn format_stdin(
    check_only: bool,
    entry: TopEntryPoint,
    options: FormatOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let formatted = format_text(&input, entry, options)?;

    if check_only {
        if input != formatted {
            eprintln!("stdin: not formatted");
            process::exit(1);
        } else {
            eprintln!("stdin: already formatted");
        }
    } else {
        print!("{}", formatted);
    }

    Ok(())
}

fn format_file_cmd(
    path: &PathBuf,
    check_only: bool,
    entry: TopEntryPoint,
    recursive_includes: bool,
    options: FormatOptions,
) -> Result<bool, Box<dyn std::error::Error>> {
    if recursive_includes {
        // Format the file and every `.fppi` fragment reachable via `include`,
        // deriving each fragment's entrypoint from its include context.
        let units = format_file_recursive(path, entry, options)?;

        let mut all_formatted = true;
        for unit in units {
            all_formatted &= apply_unit(&unit.path, &unit.original, &unit.formatted, check_only)?;
        }
        return Ok(all_formatted);
    }

    // Default: format only the named file, ignoring `include` specifiers.
    let original = fs::read_to_string(path)?;
    let formatted = format_text(&original, entry, options)?;
    apply_unit(path, &original, &formatted, check_only)
}

/// Report/write a single formatted file. Returns whether it was already
/// formatted (used to compute the overall exit status in `--check`).
fn apply_unit(
    path: &std::path::Path,
    original: &str,
    formatted: &str,
    check_only: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    let changed = original != formatted;
    if check_only {
        if changed {
            eprintln!("{}: not formatted", path.display());
            Ok(false)
        } else {
            eprintln!("{}: already formatted", path.display());
            Ok(true)
        }
    } else if changed {
        fs::write(path, formatted)?;
        eprintln!("{}: formatted", path.display());
        Ok(true)
    } else {
        eprintln!("{}: already formatted", path.display());
        Ok(true)
    }
}

fn main() {
    let args = Args::parse();

    // An explicit `--entry` wins; otherwise default to `Module`, which is
    // correct for normal `.fpp` files and `.fppi` fragments of module members.
    let entry = args.entry.unwrap_or(TopEntryPoint::Module);

    // Read from stdin when explicitly requested or when no files are given.
    // stdin has no path, so `.fpp-format` discovery starts at the current
    // working directory.
    if args.stdin || args.files.is_empty() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let options = args.resolve_options(&cwd);
        if let Err(err) = format_stdin(args.check, entry, options) {
            eprintln!("Error formatting stdin: {}", err);
            process::exit(1);
        }
        return;
    }

    // Format files. Each file's profile is resolved from the nearest
    // `.fpp-format` in its own directory tree, so a single invocation may span
    // files governed by different configs.
    let mut all_formatted = true;
    for file in &args.files {
        let dir = file.parent().unwrap_or_else(|| Path::new("."));
        let options = args.resolve_options(dir);
        match format_file_cmd(file, args.check, entry, args.recursive_includes, options) {
            Ok(formatted) => {
                if !formatted {
                    all_formatted = false;
                }
            }
            Err(err) => {
                eprintln!("Error formatting {}: {}", file.display(), err);
                all_formatted = false;
            }
        }
    }

    if !all_formatted {
        process::exit(1);
    }
}
