//! The rules file: the groups, as TOML. Documented in full in the crate README.

use crate::model::Input;
use crate::naming;
use crate::query::{self, Query};
use crate::select::Group;
use crate::{Diagnosed, RuleSet};
use fpp_core::{BytePos, SourceFile, Span};
use serde::Deserialize;
use std::ops::Range;
use toml::Spanned;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RulesFile {
    /// Match against the expanded model. File-wide, because it decides what every
    /// group in the file means.
    #[serde(default)]
    expand: bool,
    #[serde(default)]
    group: Vec<Spanned<GroupTable>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupTable {
    node: Spanned<String>,
    #[serde(rename = "where", default)]
    where_: Option<Spanned<String>>,
    #[serde(default)]
    name: Option<Spanned<String>>,
    generate: Vec<Spanned<String>>,
}

/// Read the groups out of `input`.
///
/// Must be called inside a `fpp_core::run` scope. Every problem is reported as a
/// diagnostic before returning `Err(Diagnosed)`, and as many groups as possible are
/// checked first, so one run reports every bad rule rather than the first.
pub fn load(input: &Input) -> Result<RuleSet, Diagnosed> {
    let source = &input.content;
    let file = SourceFile::new(&input.uri, source.clone());

    let parsed: RulesFile = match toml::from_str(source) {
        Ok(parsed) => parsed,
        Err(error) => {
            let range = error.span().unwrap_or(0..source.len());
            let span = span_of(file, source, &range);
            // TOML binds a bare key to the table above it, so `expand` written
            // after the first `[[group]]` is a group key -- and `deny_unknown_fields`
            // would report a correctly-spelled setting as a misspelling. The key
            // under the error's own span identifies that case without reading
            // serde's message.
            if source.get(range) == Some("expand") {
                span.error("`expand` is a file-wide setting, not a group setting")
                    .note("move it above the first `[[group]]`, where TOML will read it as one")
                    .emit();
            } else {
                span.error(error.message().to_string()).emit();
            }
            return Err(Diagnosed);
        }
    };

    if parsed.group.is_empty() {
        span_of(file, source, &(0..0))
            .error("this rules file defines no `[[group]]`")
            .note("a group is `node`, an optional `where` and `name`, and `generate`")
            .emit();
        return Err(Diagnosed);
    }

    let mut groups = Vec::with_capacity(parsed.group.len());
    let mut bad = false;
    for table in &parsed.group {
        match group(file, source, table) {
            Ok(group) => groups.push(group),
            Err(Diagnosed) => bad = true,
        }
    }
    if bad {
        return Err(Diagnosed);
    }
    Ok(RuleSet {
        groups,
        expand: parsed.expand,
    })
}

fn group(file: SourceFile, source: &str, table: &Spanned<GroupTable>) -> Result<Group, Diagnosed> {
    let raw = table.get_ref();
    let mut bad = false;

    let kind = raw.node.get_ref().clone();
    if !naming::kind_is_selectable(&kind) {
        naming::unknown_kind(value_span(file, source, &raw.node), &kind).emit();
        bad = true;
    }

    if raw.generate.is_empty() {
        span_of(file, source, &table.span())
            .error(format!("`{kind}` generates nothing"))
            .note("give it at least one `generate` suffix")
            .emit();
        bad = true;
    }
    let mut suffixes = Vec::with_capacity(raw.generate.len());
    for suffix in &raw.generate {
        match naming::check_suffix_at(value_span(file, source, suffix), suffix.get_ref()) {
            Ok(()) => suffixes.push(suffix.get_ref().clone()),
            Err(diagnostic) => {
                diagnostic.emit();
                bad = true;
            }
        }
    }

    // Both queries are attempted even when one fails, so a file with two bad
    // queries reports both.
    let where_ = optional_query(file, source, "where", raw.where_.as_ref());
    let name = optional_query(file, source, "name", raw.name.as_ref());
    if where_.is_err() || name.is_err() {
        bad = true;
    }

    if bad {
        return Err(Diagnosed);
    }
    Ok(Group {
        kind,
        where_: where_.unwrap_or(None),
        name: name.unwrap_or(None),
        suffixes,
    })
}

fn optional_query(
    file: SourceFile,
    source: &str,
    key: &str,
    value: Option<&Spanned<String>>,
) -> Result<Option<Query>, Diagnosed> {
    match value {
        None => Ok(None),
        Some(value) => parse_query(file, source, key, value).map(Some),
    }
}

/// Parse one query written in the rules file, with its spans pointing into it.
fn parse_query(
    file: SourceFile,
    source: &str,
    key: &str,
    value: &Spanned<String>,
) -> Result<Query, Diagnosed> {
    let content = content_range(source, &value.span());
    if source.get(content.clone()) != Some(value.get_ref().as_str()) {
        span_of(file, source, &value.span())
            .error(format!("`{key}` uses TOML escape sequences"))
            .note(
                "write it as a TOML literal string, in single quotes, so the query's own \
                   `\"` quoting is preserved",
            )
            .emit();
        return Err(Diagnosed);
    }
    query::parse(file, content.start, value.get_ref())
}

/// The byte range of a TOML string value's *content*. [`Spanned::span`] covers the
/// delimiters too, and a caret under those reads worse than one under the text.
fn content_range(source: &str, value: &Range<usize>) -> Range<usize> {
    let Some(raw) = source.get(value.clone()) else {
        return value.clone();
    };
    for delim in ["\"\"\"", "'''", "\"", "'"] {
        if !raw.starts_with(delim) || raw.len() < 2 * delim.len() {
            continue;
        }
        let mut start = value.start + delim.len();
        // TOML drops a newline immediately after a multi-line opening delimiter.
        if delim.len() == 3 {
            for eol in ["\r\n", "\n"] {
                if source[start..].starts_with(eol) {
                    start += eol.len();
                    break;
                }
            }
        }
        return start..value.end - delim.len();
    }
    value.clone()
}

fn value_span(file: SourceFile, source: &str, value: &Spanned<String>) -> Span {
    span_of(file, source, &content_range(source, &value.span()))
}

/// A span over `range` of `source`.
///
/// The bounds are rounded out to character boundaries: the diagnostic renderer asks
/// `line-index` for the line and column of each offset, and that panics rather than
/// erroring on one interior to a multi-byte character. A panic here would skip the
/// code that writes the `--filenames` file, turning one reportable error into a
/// CMake `file(STRINGS)` failure.
fn span_of(file: SourceFile, source: &str, range: &Range<usize>) -> Span {
    let start = char_start(source, range.start.min(source.len()));
    let end = char_end(source, range.end.max(start));
    Span::new(
        file,
        BytePos::try_from(start).unwrap_or(BytePos::MAX),
        BytePos::try_from(end - start).unwrap_or(0),
        None,
    )
}

fn char_start(text: &str, at: usize) -> usize {
    let mut start = at.min(text.len());
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    start
}

fn char_end(text: &str, at: usize) -> usize {
    let mut end = at.min(text.len());
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::{char_end, char_start, content_range};

    /// Every span rounded out here must land on a character boundary, or the
    /// diagnostic renderer panics instead of reporting the error.
    #[test]
    fn the_bounds_round_out_to_boundaries() {
        let text = "a\u{e9}b";
        assert_eq!(text.len(), 4);
        // The multi-byte character occupies bytes 1..3.
        assert_eq!(char_start(text, 2), 1);
        assert_eq!(char_end(text, 2), 3);
        assert_eq!(&text[char_start(text, 2)..char_end(text, 2)], "\u{e9}");
        // An offset already on a boundary is left alone, and one past the end is
        // clamped.
        assert_eq!(char_start(text, 1), 1);
        assert_eq!(char_end(text, 1), 1);
        assert_eq!(char_start(text, 99), 4);
        assert_eq!(char_end(text, 99), 4);
    }

    /// Every TOML string form, since the caret position depends on getting the
    /// delimiter width right.
    #[test]
    fn content_range_strips_every_delimiter() {
        let cases = [
            ("k = 'a$.b'", "a$.b"),
            ("k = \"a$.b\"", "a$.b"),
            ("k = '''a$.b'''", "a$.b"),
            ("k = \"\"\"a$.b\"\"\"", "a$.b"),
            // A newline straight after the opening delimiter is TOML's, not the
            // query's, and TOML drops it.
            ("k = '''\na$.b'''", "a$.b"),
            ("k = \"\"\"\r\na$.b\"\"\"", "a$.b"),
            // Empty strings must not underflow into the delimiters.
            ("k = ''", ""),
            ("k = \"\"", ""),
        ];
        for (source, want) in cases {
            let value = source.find('=').unwrap() + 2..source.len();
            let content = content_range(source, &value);
            assert_eq!(&source[content], want, "in {source:?}");
        }
    }

    /// The one case the caret cannot be trusted for, and which `parse_query`
    /// therefore rejects.
    #[test]
    fn an_escaped_basic_string_does_not_round_trip() {
        let source = "k = \"$@stem + \\\"_State\\\"\"";
        let value = 4..source.len();
        let decoded = "$@stem + \"_State\"";
        assert_ne!(&source[content_range(source, &value)], decoded);

        // ...while the literal-string spelling of the same query does.
        let literal = "k = '$@stem + \"_State\"'";
        let value = 4..literal.len();
        assert_eq!(&literal[content_range(literal, &value)], decoded);
    }
}
