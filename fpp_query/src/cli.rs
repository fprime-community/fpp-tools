use clap::Parser;
use std::path::PathBuf;

const LONG_ABOUT: &str = "\
List the files an FPP autocoder would generate, from the syntactic model alone.

Selects definitions from one or more FPP files and prints one path per match per
suffix. This is the configure-time (`--filenames`) half of an F Prime build
autocoder: no semantic analysis runs, no `-i` imports are needed.

This tool is meant to cover most custom autocoder cases and as a native executable
can keep the serial CMake configuration fast.

The rules are a TOML file:

    fpp-query --rules autocode.toml -d $B --filenames $B/names.txt -- *.fpp

    # autocode.toml
    [[group]]
    node = \"DefTopology\"
    where = '$.is_deployment'
    generate = [\"TopologyAc.hpp\", \"TopologyAc.cpp\"]

The syntax tree (AST) is traversed deeply and finds all nodes matching the `node` field
in all `[[group]]`. Detected nodes are filtered against an optional `where` predicate function
which should evaluate to a boolean.

QUERY LANGUAGE

  $                    the matched definition        $.is_deployment
  $.field.sub          field navigation              $.members[0].name
  $@                   all annotation lines          $@ contains \"static-tlm\"
  $@pre  $@post        just one side                 \"tag\" in $@pre
  $@kind  $@file       node kind / source file       $@file ends_with \".fppi\"
  $@line  $@included   position / came from include  !$@included
  $@scope  $@qualified enclosing scope, dotted       $@scope == \"Svc\"
  $@stem               the default filename stem     $@stem
  $^  $^Kind           parent / nearest ancestor     $^DefTopology.is_deployment

  == != < <= > >=      contains starts_with ends_with matches (glob)  in  +
  && || !              len() lower() upper() join() replace()
  literals             \"str\"  'str'  42  true  false  null

  A list on the left of contains/starts_with/ends_with/matches holds when any
  element does, which is what makes annotation matching read naturally.

  Write a query as a TOML *literal* string (single quotes), so its own `\"`
  quoting survives unescaped.

EXAMPLES

  # What would this rule set generate?
  fpp-query --rules autocode.toml -d $B top.fpp

  # What can I query? Dump the serialized model, or one kind's fields
  fpp-query --json top.fpp
  fpp-query --fields DefTopology

The crate README documents the rules file in full, and `presets/` ships the files
that reproduce upstream `fpp-filenames` byte for byte.
";

#[derive(Parser, Debug)]
#[command(name = "fpp-query", version, author, about, long_about = LONG_ABOUT)]
pub struct Args {
    /// TOML file holding the `[[group]]` rules
    #[arg(long, value_name = "FILE")]
    pub rules: Option<PathBuf>,

    /// Directory the generated files would be written to
    #[arg(
        short = 'd',
        long = "directory",
        value_name = "DIR",
        default_value = "."
    )]
    pub directory: String,

    /// Write the paths to FILE, one per line, instead of stdout. FILE is always
    /// created, even when nothing matches.
    #[arg(long, value_name = "FILE")]
    pub filenames: Option<PathBuf>,

    /// Accepted and ignored, so the same argv works for both autocoder phases:
    /// this tool is syntax-only, and an import cannot change its output.
    #[arg(short = 'i', long, value_name = "FILES", value_delimiter = ',')]
    pub imports: Vec<String>,

    /// Print the serialized syntax model as JSON and exit. This is the exact data
    /// a query sees. Pass `--rules` too for the model those rules see.
    #[arg(long)]
    pub json: bool,

    /// Print the queryable fields of KIND (or list every kind) and exit
    #[arg(long, value_name = "KIND", num_args = 0..=1, default_missing_value = "")]
    pub fields: Option<String>,

    /// FPP source files. `include` specifiers are followed; imports are not.
    #[arg(value_name = "FILES")]
    pub files: Vec<PathBuf>,
}

/// Parse argv. Usage errors are printed by clap and exit with its own code.
pub fn parse() -> Args {
    Args::parse()
}
