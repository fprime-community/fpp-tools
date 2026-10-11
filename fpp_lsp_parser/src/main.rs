use std::io::{self, Read, Write};
use std::process::ExitCode;

use fpp_lsp_parser::{TopEntryPoint, parse};

const USAGE: &str = "\
Usage: fpp_lsp_parser [--entry <entrypoint>]

Reads FPP source from stdin and writes the concrete syntax tree to stdout.

Options:
  --entry <entrypoint>   Parsing entrypoint. One of:
                           module (default), component, topology,
                           tlm-packet, tlm-packet-set, state-machine,
                           state
  -h, --help             Print this help message
";

fn run() -> Result<(), String> {
    let mut entry = TopEntryPoint::Module;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            "--entry" => {
                let name = args
                    .next()
                    .ok_or_else(|| "--entry requires an argument".to_string())?;
                entry = TopEntryPoint::from_name(&name)
                    .ok_or_else(|| format!("unknown entrypoint: {name}"))?;
            }
            other => {
                return Err(format!("unexpected argument: {other}"));
            }
        }
    }

    let mut src = String::new();
    io::stdin()
        .read_to_string(&mut src)
        .map_err(|e| format!("failed to read stdin: {e}"))?;

    let out = parse(&src, entry);
    print!("{}", out.debug_dump());
    io::stdout().flush().ok();

    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let _ = writeln!(io::stderr(), "error: {err}");
            let _ = write!(io::stderr(), "{USAGE}");
            ExitCode::FAILURE
        }
    }
}
