//! Query the FPP syntactic model and list the files an autocoder would generate.
//!
//! An F Prime build autocoder is invoked twice: once at CMake *configure* time to
//! declare which files it will generate, and once at build time to generate them.
//! The configure-time query runs once per module and needs only the syntax model.
//!
//! A field is queryable exactly when `fpp_ast` serializes it, so there is no
//! reflection layer to keep in sync with the grammar. The two things serde does not
//! reach are the kind name ([`fpp_ast::Node::KIND_NAMES`]) and the spans and
//! annotations, which live in the compiler context keyed by node handle rather than
//! in a node's fields; those are read off the live AST as the `$@...` roots.

pub mod cli;
pub mod diag;
pub mod emit;
pub mod fields;
pub mod model;
pub mod naming;
pub mod query;
pub mod rules;
pub mod select;

use select::Group;

/// A failure already reported through the diagnostic emitter. It carries no
/// message so that a caller can only decide the exit code, not print again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Diagnosed;

/// Why a run stopped: model/query diagnostics, or an environment failure.
pub enum Failed {
    /// Diagnostics were emitted.
    Diagnosed,
    /// I/O or usage failure, with a message not yet printed.
    Io(String),
}

impl From<Diagnosed> for Failed {
    fn from(_: Diagnosed) -> Failed {
        Failed::Diagnosed
    }
}

pub enum Output {
    /// The file list, ready to write.
    Paths(String),
    /// `--json` or `--fields`: text for stdout, and never the `--filenames` file.
    Report(String),
}

/// Which of the model's expansions to run before the groups are matched.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Expansions {
    /// Replace each `expand` specifier with the members of the module template it
    /// names.
    pub templates: bool,
    /// Synthesize the implicit `enum State` of each state machine that has a body.
    pub state_enums: bool,
}

impl Expansions {
    /// The model exactly as parsed.
    pub const NONE: Expansions = Expansions {
        templates: false,
        state_enums: false,
    };
}

/// The groups to match, and which model to match them against.
///
/// `expand` travels with the groups because it decides what they *mean*: rules
/// written for the expanded model ask a `DefEnum` group to pick up each state
/// machine's implicit `State` enum and a `DefComponent` group to reach the
/// components a template expansion contributes, and the same rules read against the
/// raw model would silently miss those files.
pub struct RuleSet {
    pub groups: Vec<Group>,
    pub expand: Expansions,
}

/// Everything that happens inside the one `fpp_core::run` scope.
///
/// `rules` is absent only for `--json` and `--fields` without one, which select
/// nothing and report the model as parsed.
pub fn run(
    args: &cli::Args,
    rules: Option<model::Input>,
    inputs: Vec<model::Input>,
) -> Result<Output, Failed> {
    let rule_set = match &rules {
        Some(input) => rules::load(input)?,
        None => RuleSet {
            groups: Vec::new(),
            expand: Expansions::NONE,
        },
    };

    let mut units = model::parse(inputs);
    if rule_set.expand.state_enums {
        model::add_state_enums(&mut units);
    }
    if rule_set.expand.templates {
        model::expand_templates(&mut units);
    }

    if args.json {
        let report = fields::json(&units).map_err(Failed::Io)?;
        return Ok(Output::Report(report));
    }
    if let Some(kind) = &args.fields {
        return Ok(Output::Report(fields::describe(kind, &units)));
    }

    let matches = select::run(&rule_set.groups, units.iter())?;
    Ok(Output::Paths(emit::render(
        &rule_set.groups,
        &matches,
        &args.directory,
    )))
}
