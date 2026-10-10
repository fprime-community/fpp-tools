//! The rules file: the groups, as TOML. Documented in full in the crate README.

use crate::model::Input;
use crate::naming;
use crate::query::{self, Query};
use crate::select::Group;
use crate::{Diagnosed, Expansions, RuleSet};
use fpp_core::{BytePos, SourceFile, Span};
use serde::Deserialize;
use std::fmt;
use std::ops::Range;
use toml::Spanned;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RulesFile {
    /// Which expansions to match against. File-wide, because it decides what every
    /// group in the file means.
    #[serde(default)]
    expand: Option<Spanned<ExpandValue>>,
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

/// What one name in an `expand` list does to the set being read.
type TurnOn = fn(&mut Expansions);

/// Every name an `expand` list accepts, and the expansion it turns on. The one
/// place the names are written, so the diagnostic that lists them cannot drift from
/// the set that is accepted.
const EXPANSIONS: &[(&str, TurnOn)] = &[
    ("templates", |e| e.templates = true),
    ("state-enums", |e| e.state_enums = true),
];

/// The set `expand = true` asks for: every entry of [`EXPANSIONS`], folded.
///
/// Read off the table rather than written out, so a name added there is part of
/// `expand = true` by construction. A file that asked for everything and got the set
/// as someone last spelled it would leave the new expansion off and the files it
/// contributes out of the list, which is the one failure this setting exists to
/// prevent.
fn all_expansions() -> Expansions {
    let mut all = Expansions::NONE;
    for (_, turn_on) in EXPANSIONS {
        turn_on(&mut all);
    }
    all
}

/// The `expand` value as written, before any name is checked.
///
/// Deserialization accepts every shape it can name, including the ones that are
/// errors, so that each one is reported through [`span_of`] with a caret in the
/// rules file and a list can report all of its bad names at once — a serde error
/// stops at the first.
///
/// Hence a hand-written visitor rather than `#[serde(untagged)]`: an untagged enum
/// buffers the value through serde's private `Content` type, which does not carry
/// the hook [`toml::Spanned`] reads, so the span this file needs would be gone —
/// including the one each list element keeps for its own caret.
enum ExpandValue {
    /// `expand = true`, `expand = false`: every expansion, or none.
    Every(bool),
    /// `expand = ["templates"]`: the names as written, unchecked, each with the span
    /// of the element that wrote it.
    Named(Vec<Spanned<String>>),
    /// `expand = "templates"`: rejected, because the surface has two shapes and one
    /// spelling per meaning keeps the rules grammar small. The name is not kept, since
    /// the complaint is about the shape of the value rather than about the name in it.
    Bare,
}

impl<'de> Deserialize<'de> for ExpandValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<ExpandValue, D::Error> {
        struct ExpandVisitor;

        impl<'de> serde::de::Visitor<'de> for ExpandVisitor {
            type Value = ExpandValue;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a bool or a list of expansion names")
            }

            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<ExpandValue, E> {
                Ok(ExpandValue::Every(value))
            }

            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<ExpandValue, E> {
                Ok(ExpandValue::Bare)
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<ExpandValue, A::Error> {
                let mut names = Vec::new();
                // A non-string element is serde's own error: it is one mistake, and
                // reporting it needs nothing this file knows.
                //
                // TOML honours `Spanned` at an element of a sequence, so each name
                // arrives with the range of the element that wrote it, which is what a
                // caret under one bad name among several is drawn from.
                while let Some(name) = seq.next_element::<Spanned<String>>()? {
                    names.push(name);
                }
                Ok(ExpandValue::Named(names))
            }
        }

        deserializer.deserialize_any(ExpandVisitor)
    }
}

/// Read the groups out of `input`.
///
/// Must be called inside a `fpp_core::run` scope. Every problem is reported as a
/// diagnostic before returning `Err(Diagnosed)`, and the file-wide setting and as
/// many groups as possible are checked first, so one run reports every bad rule
/// rather than the first.
pub fn load(input: &Input) -> Result<RuleSet, Diagnosed> {
    let source = &input.content;
    let file = SourceFile::new(&input.uri, source.clone());

    let parsed: RulesFile = match toml::from_str(source) {
        Ok(parsed) => parsed,
        Err(error) => {
            let range = error.span().unwrap_or(0..source.len());
            let message = error.message();
            // Two errors carry the span of an `expand` key and neither is worded as a
            // fact about the rules surface: one is serde's, about a struct field, and
            // the other is TOML's, about a document. The message tells them apart.
            let key = source.get(range.clone()) == Some("expand");
            let span = span_of(file, source, &range);
            if key && message.starts_with("unknown field") {
                // TOML binds a bare key to the table above it, so `expand` written
                // after the first `[[group]]` is a group key -- and
                // `deny_unknown_fields` would report a correctly-spelled setting as a
                // misspelling.
                span.error("`expand` is a file-wide setting, not a group setting")
                    .note("move it above the first `[[group]]`, where TOML will read it as one")
                    .emit();
            } else if key && message == "duplicate key" {
                // Two expansions read like two settings, so the key gets written once
                // per expansion. TOML takes the second as a redefinition of the first
                // rather than an addition to it, and the names belong in one list.
                span.error("`expand` is written more than once")
                    .note(format!(
                        "one `expand` names every expansion it asks for: `{}`",
                        every_expansion_list()
                    ))
                    .emit();
            } else {
                span.error(message.to_string()).emit();
            }
            return Err(Diagnosed);
        }
    };

    // The file-wide setting is read before anything can bail out, so a file that gets
    // it wrong and defines no group reports both rather than only the second.
    let expand = expansions(file, source, parsed.expand.as_ref());

    if parsed.group.is_empty() {
        span_of(file, source, &(0..0))
            .error("this rules file defines no `[[group]]`")
            .note("a group is `node`, an optional `where` and `name`, and `generate`")
            .emit();
        return Err(Diagnosed);
    }

    let mut groups = Vec::with_capacity(parsed.group.len());
    let mut bad = expand.is_err();
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
        expand: expand?,
    })
}

/// The expansions the file asks for, `Expansions::NONE` when it says nothing.
///
/// Every bad name in a list is reported, not just the first, so fixing the list
/// takes one round trip however many names it misspells.
fn expansions(
    file: SourceFile,
    source: &str,
    value: Option<&Spanned<ExpandValue>>,
) -> Result<Expansions, Diagnosed> {
    let Some(value) = value else {
        return Ok(Expansions::NONE);
    };
    let whole = value.span();

    match value.get_ref() {
        ExpandValue::Every(true) => Ok(all_expansions()),
        ExpandValue::Every(false) => Ok(Expansions::NONE),
        ExpandValue::Bare => {
            // The note spells the list out of the accepted names rather than out of
            // what was written: the value is a name this file has not checked, and a
            // suggestion built from it would be worth following only if it happened to
            // be spelled right.
            span_of(file, source, &whole)
                .error("`expand` is a list of expansions, not one name")
                .note(format!("write it as a list: `{}`", every_expansion_list()))
                .note("`expand = true` is all of them, `expand = false` none")
                .emit();
            Err(Diagnosed)
        }
        ExpandValue::Named(names) => {
            let mut expansions = Expansions::NONE;
            let mut bad = false;
            for name in names {
                if let Some((_, turn_on)) = EXPANSIONS
                    .iter()
                    .find(|(known, _)| *known == name.get_ref().as_str())
                {
                    turn_on(&mut expansions);
                    continue;
                }
                // Every element keeps its own span, so the caret lands on the name that
                // is wrong however the rest of the list is spelled -- including when it
                // reads as part of a name the list spelled right.
                //
                // An empty element has no name to quote and nothing between its quotes
                // to point at, so it is reported as the element it is, quotes included.
                let report = if name.get_ref().is_empty() {
                    span_of(file, source, &name.span()).error("this expansion name is empty")
                } else {
                    value_span(file, source, name)
                        .error(format!("`{}` is not an expansion", name.get_ref()))
                };
                report
                    .note(format!("the expansions are {}", expansion_names()))
                    .note("`expand = true` is all of them, `expand = false` none")
                    .emit();
                bad = true;
            }
            if bad { Err(Diagnosed) } else { Ok(expansions) }
        }
    }
}

/// The accepted names, as a diagnostic reads them.
fn expansion_names() -> String {
    EXPANSIONS
        .iter()
        .map(|(name, _)| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Every accepted name as one `expand` setting, for a note that shows the shape a
/// list is written in along with the names that go in it.
fn every_expansion_list() -> String {
    let names = EXPANSIONS
        .iter()
        .map(|(name, _)| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("expand = [{names}]")
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
