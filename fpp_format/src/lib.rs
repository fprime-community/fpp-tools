mod config;
#[doc(hidden)]
pub mod doc;
mod formatter;
mod include;

use std::path::{Path, PathBuf};
use std::{fs, io};

pub use fpp_lsp_parser::SyntaxError;
use fpp_lsp_parser::{TopEntryPoint, parse};

pub use crate::config::{
    CONFIG_FILE_NAME, ConfigError, PartialConfig, find_config_file, load_config, parse_config,
};
pub use crate::formatter::Formatter;
pub use crate::include::{FormattedUnit, format_file_recursive};

/// Configuration options for the formatter
#[derive(Debug, Clone)]
pub struct FormatOptions {
    /// Number of spaces per indentation level (default: 4)
    pub indent_width: usize,
    /// Maximum line width; specs wider than this explode their clauses and
    /// group-style member lists break onto multiple lines (default: 80)
    pub max_line_width: usize,
}

/// Default number of spaces per indentation level.
pub const DEFAULT_INDENT_WIDTH: usize = 4;
/// Default maximum line width.
pub const DEFAULT_MAX_LINE_WIDTH: usize = 80;

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_width: DEFAULT_INDENT_WIDTH,
            max_line_width: DEFAULT_MAX_LINE_WIDTH,
        }
    }
}

/// Errors that can occur during formatting
#[derive(Debug)]
pub enum FormatError {
    /// Parse errors occurred while parsing the input
    ParseError(Vec<SyntaxError>),
    /// I/O error occurred while reading or writing files
    IoError(io::Error),
    /// An `include` cycle was detected while formatting recursively. The vector
    /// is the include chain, ending with the file that closes the cycle.
    IncludeCycle(Vec<PathBuf>),
}

impl From<io::Error> for FormatError {
    fn from(err: io::Error) -> Self {
        FormatError::IoError(err)
    }
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::ParseError(errors) => {
                write!(f, "Parse errors: ")?;
                for err in errors {
                    write!(f, "{}, ", err)?;
                }
                Ok(())
            }
            FormatError::IoError(err) => write!(f, "I/O error: {}", err),
            FormatError::IncludeCycle(chain) => {
                write!(f, "include cycle detected: ")?;
                for (i, p) in chain.iter().enumerate() {
                    if i > 0 {
                        write!(f, " -> ")?;
                    }
                    write!(f, "{}", p.display())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for FormatError {}

/// Format FPP source text with the given options.
///
/// Returns the formatted text, or an error if the input has parse errors.
/// Note: The formatter will still attempt to format code with parse errors,
/// but may return a `ParseError` if the errors are severe.
pub fn format_text(
    text: &str,
    entry: TopEntryPoint,
    options: FormatOptions,
) -> Result<String, FormatError> {
    // Parse the input text
    let parse = parse(text, entry);

    // Check for parse errors - fail if the input cannot be parsed
    let errors = parse.errors();
    if !errors.is_empty() {
        return Err(FormatError::ParseError(errors));
    }

    // Get the syntax tree root
    let root = parse.syntax_node();

    // Create formatter and format the tree
    let formatter = Formatter::new(options);
    let formatted = formatter.format(&root);

    Ok(formatted)
}

/// Format an FPP file and return the formatted text.
///
/// This reads the file, formats it, and returns the result without
/// modifying the original file.
pub fn format_file(path: &Path, entry: TopEntryPoint) -> Result<String, FormatError> {
    let text = fs::read_to_string(path)?;
    format_text(&text, entry, FormatOptions::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_options() {
        // The default formatting profile is 4-space indent, 80-column width.
        let options = FormatOptions::default();
        assert_eq!(options.indent_width, 4);
        assert_eq!(options.max_line_width, 80);
        assert_eq!(options.indent_width, DEFAULT_INDENT_WIDTH);
        assert_eq!(options.max_line_width, DEFAULT_MAX_LINE_WIDTH);
    }

    #[test]
    fn test_indent_width_is_honored() {
        let input = "module F { constant a = 1 }";
        let two = format_text(
            input,
            TopEntryPoint::Module,
            FormatOptions {
                indent_width: 2,
                max_line_width: 80,
            },
        )
        .unwrap();
        let four = format_text(
            input,
            TopEntryPoint::Module,
            FormatOptions {
                indent_width: 4,
                max_line_width: 80,
            },
        )
        .unwrap();
        assert!(two.contains("\n  constant a = 1"), "two-space:\n{}", two);
        assert!(
            four.contains("\n    constant a = 1"),
            "four-space:\n{}",
            four
        );
    }

    #[test]
    fn test_max_line_width_is_honored() {
        // A spec that fits within a wide limit but not a narrow one explodes
        // its clauses only under the narrow width.
        let input = "module M { active component C { \
            async command StartTest(testId: U32, timeout: F32) \
            opcode 0x10 priority 5 assert } }";
        let wide = format_text(
            input,
            TopEntryPoint::Module,
            FormatOptions {
                indent_width: 2,
                max_line_width: 200,
            },
        )
        .unwrap();
        let narrow = format_text(
            input,
            TopEntryPoint::Module,
            FormatOptions {
                indent_width: 2,
                max_line_width: 40,
            },
        )
        .unwrap();
        assert!(
            !wide.contains("\\\n"),
            "wide width should not explode:\n{}",
            wide
        );
        assert!(
            narrow.contains("\\\n"),
            "narrow width should explode:\n{}",
            narrow
        );
    }

    #[test]
    fn test_format_simple_module() {
        let input = r#"module F {
    constant a = 1
    constant b = 2 + 3
}
"#;
        let result = format_text(input, TopEntryPoint::Module, FormatOptions::default());
        assert!(result.is_ok());

        let formatted = result.unwrap();
        // Verify it can be parsed again
        let parse = parse(&formatted, TopEntryPoint::Module);
        assert!(parse.errors().is_empty());
    }

    #[test]
    fn test_format_idempotent() {
        let input = "module F { constant a = 1 }";
        let result1 = format_text(input, TopEntryPoint::Module, FormatOptions::default()).unwrap();
        let result2 =
            format_text(&result1, TopEntryPoint::Module, FormatOptions::default()).unwrap();
        assert_eq!(result1, result2, "Formatter should be idempotent");
    }

    #[test]
    fn test_enum_reparses_cleanly() {
        // Phase 1: enum with comma separators must reparse after formatting
        let input = "module F { enum Color { RED, GREEN, BLUE } }";
        let formatted =
            format_text(input, TopEntryPoint::Module, FormatOptions::default()).unwrap();

        // Must reparse with zero errors
        let parse = parse(&formatted, TopEntryPoint::Module);
        assert!(
            parse.errors().is_empty(),
            "Formatted enum failed to reparse: {:?}\nFormatted output:\n{}",
            parse.errors(),
            formatted
        );

        // Must be idempotent
        let formatted2 =
            format_text(&formatted, TopEntryPoint::Module, FormatOptions::default()).unwrap();
        assert_eq!(formatted, formatted2, "Enum formatting not idempotent");
    }

    #[test]
    fn test_struct_members_separate_lines() {
        // Phase 1: struct members should be on separate lines
        let input = "module F { struct S { x: U32, y: F32 } }";
        let formatted =
            format_text(input, TopEntryPoint::Module, FormatOptions::default()).unwrap();

        // Must reparse
        let parse = parse(&formatted, TopEntryPoint::Module);
        assert!(
            parse.errors().is_empty(),
            "Formatted struct failed to reparse: {:?}\nFormatted output:\n{}",
            parse.errors(),
            formatted
        );

        // Must be idempotent
        let formatted2 =
            format_text(&formatted, TopEntryPoint::Module, FormatOptions::default()).unwrap();
        assert_eq!(formatted, formatted2, "Struct formatting not idempotent");
    }

    #[test]
    fn test_nested_module_reparses() {
        // Phase 1: nested modules with various definitions
        let input = r#"
module Outer {
  module Inner {
    constant x = 42
    enum Status { OK, ERROR }
  }
  struct Point { x: F32, y: F32 }
}
"#;
        let formatted =
            format_text(input, TopEntryPoint::Module, FormatOptions::default()).unwrap();

        // Must reparse
        let parse = parse(&formatted, TopEntryPoint::Module);
        assert!(
            parse.errors().is_empty(),
            "Formatted nested module failed to reparse: {:?}\nFormatted output:\n{}",
            parse.errors(),
            formatted
        );

        // Must be idempotent
        let formatted2 =
            format_text(&formatted, TopEntryPoint::Module, FormatOptions::default()).unwrap();
        assert_eq!(
            formatted, formatted2,
            "Nested module formatting not idempotent"
        );
    }

    #[test]
    fn test_binary_expressions_reparse() {
        // Phase 1: expressions should format and reparse correctly
        let input = "module F { constant a = 1 + 2 * 3 - 4 / 5 }";
        let formatted =
            format_text(input, TopEntryPoint::Module, FormatOptions::default()).unwrap();

        // Must reparse
        let parse = parse(&formatted, TopEntryPoint::Module);
        assert!(
            parse.errors().is_empty(),
            "Formatted expression failed to reparse: {:?}\nFormatted output:\n{}",
            parse.errors(),
            formatted
        );

        // Must be idempotent
        let formatted2 =
            format_text(&formatted, TopEntryPoint::Module, FormatOptions::default()).unwrap();
        assert_eq!(
            formatted, formatted2,
            "Expression formatting not idempotent"
        );
    }

    /// Assert that formatted output reparses cleanly and is idempotent.
    fn assert_stable(formatted: &str) {
        let parse = parse(formatted, TopEntryPoint::Module);
        assert!(
            parse.errors().is_empty(),
            "Formatted output failed to reparse: {:?}\nOutput:\n{}",
            parse.errors(),
            formatted
        );
        let again =
            format_text(formatted, TopEntryPoint::Module, FormatOptions::default()).unwrap();
        assert_eq!(formatted, again, "Formatting not idempotent");
    }

    #[test]
    fn test_long_command_explodes_clauses() {
        // A command wider than max_line_width breaks each trailing clause onto
        // its own `\`-continuation line.
        let input = "module M { active component C { \
            async command StartTest(testId: U32, timeout: F32) \
            opcode 0x10 priority 5 assert } }";
        let formatted =
            format_text(input, TopEntryPoint::Module, FormatOptions::default()).unwrap();

        assert!(
            formatted.contains("\\\n"),
            "Expected `\\` continuations for exploded clauses:\n{}",
            formatted
        );
        // Each clause on its own line.
        assert!(
            formatted.contains("opcode 0x10 \\\n"),
            "clauses:\n{}",
            formatted
        );
        assert!(
            formatted.contains("priority 5 \\\n"),
            "clauses:\n{}",
            formatted
        );
        assert_stable(&formatted);
    }

    #[test]
    fn test_blank_line_with_trailing_whitespace_is_preserved() {
        // A blank separator line that carries trailing spaces is lexed as two
        // EOL tokens around a WHITESPACE token; the blank must still be seen as
        // a single intentional separator and preserved.
        let input = "module M {\n  constant a = 1\n   \n  constant b = 2\n}\n";
        let formatted =
            format_text(input, TopEntryPoint::Module, FormatOptions::default()).unwrap();
        assert_eq!(
            formatted, "module M {\n    constant a = 1\n\n    constant b = 2\n}\n",
            "trailing-whitespace blank line not preserved:\n{}",
            formatted
        );
        // Same content without the stray spaces must format identically.
        let clean = "module M {\n    constant a = 1\n\n    constant b = 2\n}\n";
        let clean_formatted =
            format_text(clean, TopEntryPoint::Module, FormatOptions::default()).unwrap();
        assert_eq!(formatted, clean_formatted);
        assert_stable(&formatted);
    }

    #[test]
    fn test_multiple_blank_lines_with_whitespace_collapse_to_one() {
        // Several blank lines (some carrying whitespace) collapse to a single
        // separator, just like clean blank lines do.
        let input = "module M {\n  constant a = 1\n \n\n  \n  constant b = 2\n}\n";
        let formatted =
            format_text(input, TopEntryPoint::Module, FormatOptions::default()).unwrap();
        assert_eq!(
            formatted, "module M {\n    constant a = 1\n\n    constant b = 2\n}\n",
            "blank run not collapsed to one:\n{}",
            formatted
        );
        assert_stable(&formatted);
    }

    #[test]
    fn test_short_spec_stays_inline() {
        // A spec under the width limit keeps its clauses on one line.
        let input = "module M { active component C { \
            sync command Stop opcode 0x1 } }";
        let formatted =
            format_text(input, TopEntryPoint::Module, FormatOptions::default()).unwrap();
        assert!(
            !formatted.contains('\\'),
            "Short spec should not be exploded:\n{}",
            formatted
        );
        assert_stable(&formatted);
    }

    #[test]
    fn test_connection_arrows_align() {
        let input = "module M { topology T { \
            connections C { a.longOutputName -> b.i, c.x -> d.y } } }";
        let formatted =
            format_text(input, TopEntryPoint::Module, FormatOptions::default()).unwrap();

        // Both arrows land in the same column.
        let cols: Vec<usize> = formatted
            .lines()
            .filter(|l| l.contains("->"))
            .map(|l| l.find("->").unwrap())
            .collect();
        assert_eq!(cols.len(), 2, "expected two connections:\n{}", formatted);
        assert_eq!(cols[0], cols[1], "arrows not aligned:\n{}", formatted);
        assert_stable(&formatted);
    }

    #[test]
    fn test_telemetry_limits_indent() {
        // Limit-sequence members indent one level past their `low {` brace.
        let input = "module M { active component C { \
            telemetry t: F32 low { yellow 1.0, red 2.0 } } }";
        let formatted =
            format_text(input, TopEntryPoint::Module, FormatOptions::default()).unwrap();
        let indent = |needle: &str| -> usize {
            let line = formatted
                .lines()
                .find(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("missing {:?} in:\n{}", needle, formatted));
            line.len() - line.trim_start().len()
        };
        assert!(
            indent("yellow 1.0") > indent("low {"),
            "limit members should be indented past `low`:\n{}",
            formatted
        );
        assert_stable(&formatted);
    }

    #[test]
    fn test_comment_before_choice_if_not_swallowed() {
        // A standalone comment inside a choice body must stay on its own line;
        // otherwise the following `if` is swallowed by the `#` comment and the
        // output fails to reparse.
        let input = r#"module M {
  state machine S {
    choice C {
      # leading comment
      if g enter A else enter B
    }
  }
}
"#;
        let formatted =
            format_text(input, TopEntryPoint::Module, FormatOptions::default()).unwrap();
        assert!(
            formatted.contains("# leading comment\n"),
            "comment must be followed by a newline:\n{}",
            formatted
        );
        assert_stable(&formatted);
    }

    #[test]
    fn test_trailing_newline() {
        // Phase 1: exactly one trailing newline
        let input = "module F { constant a = 1 }";
        let formatted =
            format_text(input, TopEntryPoint::Module, FormatOptions::default()).unwrap();

        assert!(formatted.ends_with('\n'), "Missing trailing newline");
        assert!(
            !formatted.ends_with("\n\n"),
            "Multiple trailing newlines: {:?}",
            formatted.chars().rev().take(5).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_idempotent_all_fixtures() {
        // Phase 1: comprehensive idempotency test
        let test_cases = vec![
            "module F { }",
            "module F { constant a = 1 }",
            "module F { enum E { A, B, C } }",
            "module F { struct S { x: U32 } }",
            "module F { struct S { x: U32, y: F32, z: Bool } }",
            "module F { enum E { A } }",
            "module F { module Inner { constant x = 1 } }",
        ];

        for input in test_cases {
            let formatted1 =
                format_text(input, TopEntryPoint::Module, FormatOptions::default()).unwrap();
            let formatted2 =
                format_text(&formatted1, TopEntryPoint::Module, FormatOptions::default()).unwrap();
            assert_eq!(
                formatted1, formatted2,
                "Not idempotent for input: {}\nFirst:\n{}\nSecond:\n{}",
                input, formatted1, formatted2
            );

            // Also verify it reparses
            let parse = parse(&formatted1, TopEntryPoint::Module);
            assert!(
                parse.errors().is_empty(),
                "Failed to reparse for input: {}\nFormatted:\n{}\nErrors: {:?}",
                input,
                formatted1,
                parse.errors()
            );
        }
    }

    #[test]
    fn test_all_fixtures_reparse_and_idempotent() {
        // Test all fixtures in tests/ directory
        let fixtures = vec![
            // Original fixtures
            "simple-module.fpp",
            "nested-module.fpp",
            "comments.fpp",
            "multiple-definitions.fpp",
            "binary-expressions.fpp",
            // New fixtures
            "annotations.fpp",
            "array-struct.fpp",
            "component.fpp",
            "port.fpp",
            "topology.fpp",
            "state-machine.fpp",
            // Coverage-driven fixtures for previously-untested constructs
            "instances-locate.fpp",
            "interface-include.fpp",
            "expressions.fpp",
            "type-defs.fpp",
            "topology-extra.fpp",
            "component-extra.fpp",
        ];

        for fixture_name in fixtures {
            let fixture_path = format!("tests/{}", fixture_name);
            let input = match fs::read_to_string(&fixture_path) {
                Ok(content) => content,
                Err(_) => {
                    // Fixture might not exist yet, skip
                    continue;
                }
            };

            // First pass formatting
            let formatted1 = format_text(&input, TopEntryPoint::Module, FormatOptions::default())
                .unwrap_or_else(|e| panic!("{}: Format failed: {:?}", fixture_name, e));

            // CRITICAL: Must parse cleanly with fpp_lsp_parser (not fpp binary!)
            let parse_result = parse(&formatted1, TopEntryPoint::Module);
            if !parse_result.errors().is_empty() {
                eprintln!(
                    "{}: {} LSP parse errors",
                    fixture_name,
                    parse_result.errors().len()
                );
                for (i, err) in parse_result.errors().iter().take(3).enumerate() {
                    eprintln!("  {}. {:?}", i + 1, err);
                }
            }
            assert!(
                parse_result.errors().is_empty(),
                "{}: First-pass output has LSP parse errors: {:?}\nOutput:\n{}",
                fixture_name,
                parse_result.errors(),
                formatted1
            );

            // Second pass - test idempotency
            let formatted2 =
                format_text(&formatted1, TopEntryPoint::Module, FormatOptions::default())
                    .unwrap_or_else(|e| panic!("{}: Second format failed: {:?}", fixture_name, e));

            assert_eq!(
                formatted1, formatted2,
                "{}: Not idempotent\nFirst:\n{}\nSecond:\n{}",
                fixture_name, formatted1, formatted2
            );
        }
    }
}
