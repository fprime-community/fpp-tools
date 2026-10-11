//! Recursive formatting of `include` fragments (`.fppi`).
//!
//! FPP models are split across files with `include "path"` specifiers. An
//! included fragment is *not* a standalone module: it is spliced into the body
//! of whatever construct contains the `include`, so it must be parsed (and
//! therefore formatted) with the entrypoint of that context. For example, an
//! `include` inside a `component { ... }` body is a sequence of component
//! members and must be formatted with [`TopEntryPoint::Component`].
//!
//! This module walks the syntax tree of a formatted file, finds every
//! `include`, derives the correct entrypoint from the include's context,
//! resolves the path relative to the including file (mirroring
//! `fpp_parser::include`), and recurses. A DFS ancestor chain provides cycle
//! detection and a visited set avoids formatting a shared fragment twice.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use fpp_lsp_parser::{SyntaxKind::*, SyntaxNode, TopEntryPoint, parse};

use crate::{FormatError, FormatOptions, Formatter, restore_line_endings};

/// A single file processed by [`format_file_recursive`].
#[derive(Debug)]
pub struct FormattedUnit {
    /// Path to the file, as reached from the root (relative includes preserved).
    pub path: PathBuf,
    /// Original file contents.
    pub original: String,
    /// Formatted file contents.
    pub formatted: String,
    /// Entrypoint used to parse/format this file (derived from include context).
    pub entry: TopEntryPoint,
}

impl FormattedUnit {
    /// Whether formatting changed the file.
    pub fn changed(&self) -> bool {
        self.original != self.formatted
    }
}

/// Format `path` with `entry`, then recursively discover and format every file
/// reachable through `include` specifiers.
///
/// The returned units are ordered root-first (pre-order over the include tree),
/// with each file appearing exactly once.
pub fn format_file_recursive(
    path: &Path,
    entry: TopEntryPoint,
    options: FormatOptions,
) -> Result<Vec<FormattedUnit>, FormatError> {
    let mut results = Vec::new();
    let mut chain: Vec<PathBuf> = Vec::new();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    format_recursive_inner(
        path,
        entry,
        &options,
        &mut results,
        &mut chain,
        &mut visited,
    )?;
    Ok(results)
}

fn format_recursive_inner(
    path: &Path,
    entry: TopEntryPoint,
    options: &FormatOptions,
    results: &mut Vec<FormattedUnit>,
    chain: &mut Vec<PathBuf>,
    visited: &mut HashSet<PathBuf>,
) -> Result<(), FormatError> {
    // Canonicalize for identity; this also surfaces missing files early.
    let canon = path.canonicalize()?;

    if chain.contains(&canon) {
        let mut cycle = chain.clone();
        cycle.push(canon);
        return Err(FormatError::IncludeCycle(cycle));
    }
    if !visited.insert(canon.clone()) {
        // Already formatted via another include path; not a cycle.
        return Ok(());
    }

    let original = std::fs::read_to_string(path)?;
    let parse = parse(&original, entry);
    let errors = parse.errors();
    if !errors.is_empty() {
        return Err(FormatError::ParseError(errors));
    }
    let root = parse.syntax_node();
    let formatted = restore_line_endings(&original, Formatter::new(options.clone()).format(&root));

    results.push(FormattedUnit {
        path: path.to_path_buf(),
        original,
        formatted,
        entry,
    });

    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    chain.push(canon);
    for spec in root.descendants().filter(|n| n.kind() == SPEC_INCLUDE) {
        let Some((rel, inc_entry)) = include_target(&spec, entry) else {
            continue;
        };
        let resolved = dir.join(&rel);
        format_recursive_inner(&resolved, inc_entry, options, results, chain, visited)?;
    }
    chain.pop();

    Ok(())
}

/// The include path string and the entrypoint derived from its context, or
/// `None` when the include has no path. `file_entry` is the entrypoint of the
/// file that contains the include.
fn include_target(spec: &SyntaxNode, file_entry: TopEntryPoint) -> Option<(String, TopEntryPoint)> {
    let entry = context_entry(spec, file_entry);
    let lit = spec
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == LITERAL_STRING)?;
    Some((strip_quotes(lit.text()), entry))
}

/// Walk up the ancestors of a `SPEC_INCLUDE` to find the enclosing member list
/// and map it to the parser entrypoint used for that kind of fragment. An
/// include with no enclosing member list is a member of the file that
/// contains it, so its fragment has the same entrypoint as that file.
fn context_entry(spec: &SyntaxNode, file_entry: TopEntryPoint) -> TopEntryPoint {
    let mut node = spec.parent();
    while let Some(n) = node {
        match n.kind() {
            MODULE_MEMBER_LIST => return TopEntryPoint::Module,
            COMPONENT_MEMBER_LIST => return TopEntryPoint::Component,
            TOPOLOGY_MEMBER_LIST => return TopEntryPoint::Topology,
            TLM_PACKET_MEMBER_LIST => return TopEntryPoint::TlmPacket,
            TLM_PACKET_SET_MEMBER_LIST => return TopEntryPoint::TlmPacketSet,
            STATE_MACHINE_MEMBER_LIST => return TopEntryPoint::StateMachine,
            STATE_MEMBER_LIST => return TopEntryPoint::State,
            _ => {}
        }
        node = n.parent();
    }
    file_entry
}

/// Strip the surrounding quotes from an FPP string literal.
fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if let Some(inner) = s
        .strip_prefix("\"\"\"")
        .and_then(|x| x.strip_suffix("\"\"\""))
    {
        return inner.trim().to_string();
    }
    if let Some(inner) = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
        return inner.to_string();
    }
    s.to_string()
}
