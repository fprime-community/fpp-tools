//! Wadler/Prettier-style document IR and renderer for the FPP formatter.
//!
//! Lowering (see `formatter`) turns a syntax tree into a `Doc`; `render`
//! decides flat-vs-broken for each `Group` based on the max line width and
//! emits the final text. A post-pass aligns grouped `@<` and `->` anchors.

use std::rc::Rc;

/// The kind of a column-alignment anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnchorKind {
    /// A `@<` post-annotation.
    PostAnnotation,
    /// A `->` in a topology direct connection.
    Arrow,
    /// An `=` in a constant definition.
    Equals,
}

/// Document IR node.
#[derive(Clone)]
pub enum Doc {
    Nil,
    Text(Rc<str>),
    /// Flexible break: `flat` text when its group is flat, otherwise a newline
    /// (prefixed with ` \` when `cont`) plus indentation.
    Line {
        flat: &'static str,
        cont: bool,
    },
    /// Unconditional break (prefixed with ` \` when `cont`).
    Hard {
        cont: bool,
    },
    /// Indent the child by `n` levels.
    Nest(i32, Rc<Doc>),
    Concat(Rc<[Doc]>),
    /// Render flat if it fits within the width, else broken.
    Group(Rc<Doc>),
    /// Record a column-alignment anchor at the child's start column.
    Anchor(AnchorKind, Rc<Doc>),
}

impl Doc {
    pub fn text(s: impl Into<String>) -> Doc {
        Doc::Text(Rc::from(s.into().as_str()))
    }
    pub fn concat(v: Vec<Doc>) -> Doc {
        Doc::Concat(Rc::from(v))
    }
    pub fn nest(n: i32, d: Doc) -> Doc {
        Doc::Nest(n, Rc::new(d))
    }
    pub fn group(d: Doc) -> Doc {
        Doc::Group(Rc::new(d))
    }
    pub fn anchor(kind: AnchorKind, d: Doc) -> Doc {
        Doc::Anchor(kind, Rc::new(d))
    }
    /// Flat = single space, break = newline.
    pub fn softline() -> Doc {
        Doc::Line {
            flat: " ",
            cont: false,
        }
    }
    /// Flat = nothing, break = newline.
    pub fn softline_tight() -> Doc {
        Doc::Line {
            flat: "",
            cont: false,
        }
    }
    /// Flat = single space, break = ` \` continuation newline.
    pub fn contline() -> Doc {
        Doc::Line {
            flat: " ",
            cont: true,
        }
    }
    pub fn hardline() -> Doc {
        Doc::Hard { cont: false }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Flat,
    Break,
}

/// A recorded alignment anchor position in the rendered output.
struct Anchor {
    line: usize,
    col: usize,
    level: i32,
    kind: AnchorKind,
}

/// Render `doc` to final formatted text.
pub fn render(doc: &Doc, width: usize, indent_width: usize) -> String {
    let mut out = String::new();
    let mut col: usize = 0;
    let mut line: usize = 0;
    let mut anchors: Vec<Anchor> = Vec::new();
    // Lines whose end lies inside a text spanning several lines (a multi-line
    // string literal); their trailing spaces belong to the text.
    let mut verbatim_lines: Vec<usize> = Vec::new();
    let mut stack: Vec<(i32, Mode, &Doc)> = vec![(0, Mode::Break, doc)];

    while let Some((ind, mode, d)) = stack.pop() {
        match d {
            Doc::Nil => {}
            Doc::Text(s) => {
                out.push_str(s);
                if let Some(nl) = s.rfind('\n') {
                    let newlines = s.matches('\n').count();
                    verbatim_lines.extend(line..line + newlines);
                    line += newlines;
                    col = s[nl + 1..].chars().count();
                } else {
                    col += s.chars().count();
                }
            }
            Doc::Anchor(kind, x) => {
                anchors.push(Anchor {
                    line,
                    col,
                    level: ind,
                    kind: *kind,
                });
                stack.push((ind, mode, x));
            }
            Doc::Concat(v) => {
                for c in v.iter().rev() {
                    stack.push((ind, mode, c));
                }
            }
            Doc::Nest(n, x) => stack.push((ind + n, mode, x)),
            Doc::Group(x) => {
                let m = if fits(width as isize - col as isize, x, &stack) {
                    Mode::Flat
                } else {
                    Mode::Break
                };
                stack.push((ind, m, x));
            }
            Doc::Line { flat, cont } => match mode {
                Mode::Flat => {
                    out.push_str(flat);
                    col += flat.chars().count();
                }
                Mode::Break => {
                    emit_break(&mut out, &mut col, &mut line, *cont, ind, indent_width);
                }
            },
            Doc::Hard { cont } => {
                emit_break(&mut out, &mut col, &mut line, *cont, ind, indent_width);
            }
        }
    }

    let aligned = align(&out, &anchors, indent_width);
    finish(aligned, &verbatim_lines)
}

fn emit_break(
    out: &mut String,
    col: &mut usize,
    line: &mut usize,
    cont: bool,
    ind: i32,
    iw: usize,
) {
    if cont {
        out.push_str(" \\");
    }
    out.push('\n');
    *line += 1;
    let spaces = ind.max(0) as usize * iw;
    out.push_str(&" ".repeat(spaces));
    *col = spaces;
}

/// Whether `x` (rendered flat) plus the continuation `rest` fits on the line.
fn fits(mut rem: isize, x: &Doc, rest: &[(i32, Mode, &Doc)]) -> bool {
    let mut work: Vec<(Mode, &Doc)> = Vec::with_capacity(rest.len() + 1);
    for (_, m, d) in rest.iter() {
        work.push((*m, d));
    }
    work.push((Mode::Flat, x));

    while rem >= 0 {
        let (m, d) = match work.pop() {
            Some(v) => v,
            None => return true,
        };
        match d {
            Doc::Nil => {}
            Doc::Text(s) => {
                if let Some(nl) = s.find('\n') {
                    rem -= s[..nl].chars().count() as isize;
                    return rem >= 0;
                }
                rem -= s.chars().count() as isize;
            }
            Doc::Anchor(_, y) => work.push((m, y)),
            Doc::Concat(v) => {
                for c in v.iter().rev() {
                    work.push((m, c));
                }
            }
            Doc::Nest(_, y) => work.push((m, y)),
            Doc::Group(y) => work.push((Mode::Flat, y)),
            Doc::Line { flat, .. } => match m {
                Mode::Flat => rem -= flat.chars().count() as isize,
                Mode::Break => return true,
            },
            Doc::Hard { .. } => match m {
                Mode::Flat => return false,
                Mode::Break => return true,
            },
        }
    }
    false
}

/// Left-pad anchor tokens so contiguous same-level groups share a column.
///
/// A single line can carry more than one anchor kind — an enum constant is
/// `NAME = value @< doc`, contributing both an `Equals` and a `PostAnnotation`
/// anchor. Because the renderer records anchors in traversal order, the two
/// kinds interleave in `anchors` (`Equals`, `PostAnnotation`, `Equals`, ...),
/// so runs cannot be found by vector adjacency alone. Instead we bucket by
/// `(kind, level)` — within a bucket the indices are already in ascending line
/// order — then split each bucket into maximal consecutive-line runs.
///
/// Groups are then applied left-to-right (by column): padding inserted before a
/// leftward anchor (e.g. `=`) shifts the recorded column of every rightward
/// anchor on the same line (e.g. `@<`), so `shift` tracks the running per-line
/// offset and later groups align against the post-shift columns.
fn align(text: &str, anchors: &[Anchor], iw: usize) -> String {
    if anchors.is_empty() {
        return text.to_string();
    }
    let _ = iw;
    let mut lines: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();

    // Bucket anchor indices by (kind, level). Traversal visits anchors
    // top-to-bottom, so each bucket is already sorted by line.
    let mut buckets: std::collections::BTreeMap<(AnchorKind, i32), Vec<usize>> =
        std::collections::BTreeMap::new();
    for (idx, a) in anchors.iter().enumerate() {
        buckets.entry((a.kind, a.level)).or_default().push(idx);
    }

    // Split each bucket into maximal runs of consecutive lines.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for idxs in buckets.into_values() {
        let mut run: Vec<usize> = Vec::new();
        for idx in idxs {
            if let Some(&last) = run.last()
                && anchors[idx].line != anchors[last].line + 1
            {
                groups.push(std::mem::take(&mut run));
            }
            run.push(idx);
        }
        if !run.is_empty() {
            groups.push(run);
        }
    }

    // Apply leftmost anchors first so their padding is reflected in the shift
    // seen by rightward anchors sharing the same lines.
    groups.sort_by_key(|g| anchors[g[0]].col);

    let mut shift: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for g in &groups {
        let target = g
            .iter()
            .map(|&idx| anchors[idx].col + shift.get(&anchors[idx].line).copied().unwrap_or(0))
            .max()
            .unwrap_or(0);
        for &idx in g {
            let a = &anchors[idx];
            let at = a.col + shift.get(&a.line).copied().unwrap_or(0);
            if at < target
                && let Some(l) = lines.get_mut(a.line)
            {
                let byte = l.char_indices().nth(at).map(|(b, _)| b).unwrap_or(l.len());
                l.insert_str(byte, &" ".repeat(target - at));
                *shift.entry(a.line).or_insert(0) += target - at;
            }
        }
    }
    lines.join("\n")
}

/// Trim trailing whitespace on each line, except the lines listed in
/// `verbatim_lines`, and ensure exactly one final newline.
fn finish(text: String, verbatim_lines: &[usize]) -> String {
    let mut out: String = text
        .split('\n')
        .enumerate()
        .map(|(i, l)| {
            if verbatim_lines.binary_search(&i).is_ok() {
                l
            } else {
                l.trim_end()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}
