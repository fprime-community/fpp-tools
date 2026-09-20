//! `fpp-query` — list the files an FPP autocoder would generate.

use fpp_query::{Failed, Output, cli, diag, emit, model};
use std::process::exit;

fn main() {
    let args = cli::parse();

    // `--json` and `--fields` report the model rather than selecting from it, so
    // they are the only modes that work without rules.
    let reporting = args.json || args.fields.is_some();
    if args.rules.is_none() && !reporting {
        fail_usage("no rules given: pass `--rules <FILE>`");
    }

    // Reading happens before the compiler context is installed, so an unreadable
    // path fails before any compiler state exists.
    let rules = match &args.rules {
        Some(path) => match model::read_one(path) {
            Ok(input) => Some(input),
            Err(message) => fail_usage(&message),
        },
        None => None,
    };

    // `--fields` is the one mode that is useful with no input at all: it answers
    // from the kind registry, and says so when it needed a node and had none.
    let needs_input = !(args.fields.is_some() && args.files.is_empty());
    let inputs = if needs_input {
        match model::read(&args.files) {
            Ok(inputs) => inputs,
            Err(message) => fail_usage(&message),
        }
    } else {
        Vec::new()
    };

    let mut emitter = diag::StderrEmitter::new();
    let mut ctx = fpp_core::CompilerContext::new(&mut emitter);
    let result = fpp_core::run(&mut ctx, || fpp_query::run(&args, rules, inputs));

    match result {
        Ok(Output::Report(text)) => {
            if emitter.has_errors() {
                exit(1);
            }
            print!("{text}");
        }
        Ok(Output::Paths(text)) => {
            // The file is written even when the model had errors, and before the
            // exit-code decision, because `file(STRINGS)` on a missing file is a
            // hard CMake failure.
            // A missing file turns one diagnosable error into two.
            if let Some(path) = &args.filenames
                && let Err(message) = emit::write(path, &text)
            {
                eprintln!("fpp-query: {message}");
                exit(2);
            }
            if emitter.has_errors() {
                exit(1);
            }
            if args.filenames.is_none() {
                print!("{text}");
            }
        }
        Err(Failed::Diagnosed) => {
            // Leave a well-formed (empty) list behind so CMake
            // reports our diagnostic rather than its own read failure.
            if let Some(path) = &args.filenames {
                let _ = emit::write(path, "");
            }
            exit(1);
        }
        Err(Failed::Io(message)) => {
            eprintln!("fpp-query: {message}");
            exit(2);
        }
    }
}

fn fail_usage(message: &str) -> ! {
    eprintln!("fpp-query: {message}");
    eprintln!("try `fpp-query --help`");
    exit(2)
}
