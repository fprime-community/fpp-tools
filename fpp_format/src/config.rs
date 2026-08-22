//! Discovery and parsing of `.fpp-format` configuration files.
//!
//! A `.fpp-format` file declares the formatting profile (indentation width and
//! maximum line length) for a project, mirroring how `.clang-format` works for
//! C/C++. Discovery walks up the directory tree from a starting point until a
//! `.fpp-format` file is found, so every consumer of the formatter — the
//! `fpp-format` CLI, `fprime-util format`, and the language server's
//! format-on-save — resolves the same profile for a given file.
//!
//! The file format is a minimal `key = value` list. Blank lines and `#`
//! comments are ignored. Supported keys:
//!
//! ```text
//! # Spaces per indentation level
//! indent = 4
//! # Maximum line width before specs explode their clauses
//! line-length = 80
//! ```
//!
//! Precedence, lowest to highest: built-in defaults, then the `.fpp-format`
//! file, then explicit command-line overrides. This layering is expressed by
//! [`PartialConfig::merge`] and [`PartialConfig::into_options`].

use std::fmt;
use std::path::{Path, PathBuf};

use crate::FormatOptions;

/// Name of the configuration file discovered by [`find_config_file`].
pub const CONFIG_FILE_NAME: &str = ".fpp-format";

/// A formatting profile with each field optional. Fields left `None` fall
/// through to the next-lower precedence source when resolved.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PartialConfig {
    /// Spaces per indentation level, if specified.
    pub indent_width: Option<usize>,
    /// Maximum line width, if specified.
    pub max_line_width: Option<usize>,
}

impl PartialConfig {
    /// Layer `higher` on top of `self`: any field set in `higher` wins, while
    /// fields it leaves `None` keep this config's value.
    pub fn merge(self, higher: PartialConfig) -> PartialConfig {
        PartialConfig {
            indent_width: higher.indent_width.or(self.indent_width),
            max_line_width: higher.max_line_width.or(self.max_line_width),
        }
    }

    /// Resolve into concrete [`FormatOptions`], filling any unset field with the
    /// formatter's built-in default.
    pub fn into_options(self) -> FormatOptions {
        let defaults = FormatOptions::default();
        FormatOptions {
            indent_width: self.indent_width.unwrap_or(defaults.indent_width),
            max_line_width: self.max_line_width.unwrap_or(defaults.max_line_width),
        }
    }
}

/// An error encountered while parsing a `.fpp-format` file.
#[derive(Debug)]
pub struct ConfigError {
    /// Path of the offending config file, if known.
    pub path: Option<PathBuf>,
    /// 1-based line number of the offending entry.
    pub line: usize,
    /// Human-readable description of the problem.
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(path) => write!(f, "{}:{}: {}", path.display(), self.line, self.message),
            None => write!(f, "line {}: {}", self.line, self.message),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Parse the textual contents of a `.fpp-format` file.
///
/// `path` is used only to enrich error messages; pass `None` when parsing text
/// that is not backed by a file on disk.
pub fn parse_config(text: &str, path: Option<&Path>) -> Result<PartialConfig, ConfigError> {
    let mut config = PartialConfig::default();

    for (index, raw_line) in text.lines().enumerate() {
        let line_no = index + 1;
        // Strip `#` comments, then surrounding whitespace.
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        let (key, value) = line.split_once('=').ok_or_else(|| ConfigError {
            path: path.map(Path::to_path_buf),
            line: line_no,
            message: format!("expected 'key = value', found '{}'", line),
        })?;
        let key = key.trim();
        let value = value.trim();

        let parse_usize = |field: &str| -> Result<usize, ConfigError> {
            value.parse::<usize>().map_err(|_| ConfigError {
                path: path.map(Path::to_path_buf),
                line: line_no,
                message: format!(
                    "invalid value '{}' for '{}' (expected a non-negative integer)",
                    value, field
                ),
            })
        };

        match key {
            "indent" => config.indent_width = Some(parse_usize("indent")?),
            "line-length" => config.max_line_width = Some(parse_usize("line-length")?),
            other => {
                return Err(ConfigError {
                    path: path.map(Path::to_path_buf),
                    line: line_no,
                    message: format!(
                        "unknown key '{}' (supported keys: indent, line-length)",
                        other
                    ),
                });
            }
        }
    }

    Ok(config)
}

/// Walk up from `start_dir` (inclusive) to the filesystem root, returning the
/// path of the first `.fpp-format` file found, or `None` if none exists.
pub fn find_config_file(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = Some(start_dir);
    while let Some(current) = dir {
        let candidate = current.join(CONFIG_FILE_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = current.parent();
    }
    None
}

/// Discover and parse the `.fpp-format` file governing `start_dir`.
///
/// Returns an empty [`PartialConfig`] when no config file is found (so callers
/// fall back to defaults), or a parse error when a file is found but invalid.
pub fn load_config(start_dir: &Path) -> Result<PartialConfig, ConfigError> {
    let Some(path) = find_config_file(start_dir) else {
        return Ok(PartialConfig::default());
    };
    let text = std::fs::read_to_string(&path).map_err(|err| ConfigError {
        path: Some(path.clone()),
        line: 0,
        message: format!("could not read config file: {}", err),
    })?;
    parse_config(&text, Some(&path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_keys() {
        let cfg = parse_config("indent = 4\nline-length = 100\n", None).unwrap();
        assert_eq!(cfg.indent_width, Some(4));
        assert_eq!(cfg.max_line_width, Some(100));
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        let text = "# profile\n\n  indent = 2  # two spaces\n\n";
        let cfg = parse_config(text, None).unwrap();
        assert_eq!(cfg.indent_width, Some(2));
        assert_eq!(cfg.max_line_width, None);
    }

    #[test]
    fn rejects_unknown_key() {
        let err = parse_config("tabs = 4\n", None).unwrap_err();
        assert_eq!(err.line, 1);
        assert!(err.message.contains("unknown key"), "{}", err.message);
    }

    #[test]
    fn rejects_non_integer_value() {
        let err = parse_config("indent = wide\n", None).unwrap_err();
        assert_eq!(err.line, 1);
        assert!(err.message.contains("invalid value"), "{}", err.message);
    }

    #[test]
    fn rejects_line_without_equals() {
        let err = parse_config("indent 4\n", None).unwrap_err();
        assert_eq!(err.line, 1);
    }

    #[test]
    fn merge_prefers_higher_precedence() {
        let file = PartialConfig {
            indent_width: Some(4),
            max_line_width: Some(100),
        };
        let cli = PartialConfig {
            indent_width: Some(2),
            max_line_width: None,
        };
        let merged = file.merge(cli);
        // CLI indent wins; line length falls through to the file value.
        assert_eq!(merged.indent_width, Some(2));
        assert_eq!(merged.max_line_width, Some(100));
    }

    #[test]
    fn into_options_fills_defaults() {
        let opts = PartialConfig::default().into_options();
        let defaults = FormatOptions::default();
        assert_eq!(opts.indent_width, defaults.indent_width);
        assert_eq!(opts.max_line_width, defaults.max_line_width);
    }
}
