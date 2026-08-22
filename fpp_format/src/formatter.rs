//! FPP formatter: lowers the lossless syntax tree to a `Doc` (see `doc.rs`)
//! and renders it. Structure drives formatting via recursion, so there is no
//! stateful bookkeeping: indentation, member breaks and clause explosion all
//! fall out of the `Doc` combinators.

// The classification predicates below are intentionally written as explicit
// `match ... => true, _ => false` expressions for readability over `matches!`.
#![allow(clippy::match_like_matches_macro)]

use crate::FormatOptions;
use crate::doc::{AnchorKind, Doc};
use fpp_lsp_parser::{SyntaxKind, SyntaxKind::*, SyntaxNode, SyntaxToken};

pub struct Formatter {
    width: usize,
    indent_width: usize,
}

impl Formatter {
    pub fn new(options: FormatOptions) -> Self {
        Self {
            width: options.max_line_width,
            indent_width: options.indent_width,
        }
    }

    pub fn format(&self, root: &SyntaxNode) -> String {
        let doc = self.lower_container(root);
        crate::doc::render(&doc, self.width, self.indent_width)
    }

    // ---- containers (ROOT / member lists) ------------------------------

    fn lower_container(&self, node: &SyntaxNode) -> Doc {
        join_items(&self.collect_items(node).items)
    }

    /// Collect member items from a container, attaching trailing comments /
    /// post-annotations to the preceding item and tracking blank lines.
    fn collect_items(&self, node: &SyntaxNode) -> Collected {
        let mut items: Vec<Item> = Vec::new();
        let mut saw_eol = false;
        let mut blank = false;
        let mut started = false;
        // Whether a blank line separates the last member from the closing
        // delimiter (used to preserve a trailing blank inside a module body).
        let mut trailing_blank = false;
        // Newlines seen in the current run of trivia since the last real item.
        // The lexer splits a blank line that carries trailing whitespace into
        // separate EOL tokens (`"\n"` WHITESPACE `"\n"`), so a blank line must
        // be detected by accumulating newlines across EOL tokens rather than
        // looking for two newlines in a single token. WHITESPACE / COMMA / SEMI
        // are transparent and do not reset the run.
        let mut newlines = 0usize;

        for child in node.children_with_tokens() {
            if let Some(t) = child.as_token() {
                match t.kind() {
                    WHITESPACE | COMMA | SEMI => {}
                    EOL => {
                        newlines += t.text().matches('\n').count();
                        if started && newlines >= 2 {
                            blank = true;
                        }
                        saw_eol = true;
                    }
                    LEFT_CURLY | LEFT_PAREN | LEFT_SQUARE => started = true,
                    RIGHT_CURLY | RIGHT_PAREN | RIGHT_SQUARE => {
                        if started && newlines >= 2 {
                            trailing_blank = true;
                        }
                    }
                    COMMENT => {
                        if saw_eol || items.is_empty() {
                            items.push(Item::line_swallowing(Doc::text(t.text()), blank));
                        } else {
                            attach(&mut items, Doc::text(t.text()), " ");
                        }
                        saw_eol = false;
                        blank = false;
                        newlines = 0;
                        started = true;
                    }
                    PRE_ANNOTATION => {
                        items.push(Item::line_swallowing(Doc::text(t.text()), blank));
                        saw_eol = false;
                        blank = false;
                        newlines = 0;
                        started = true;
                    }
                    POST_ANNOTATION => {
                        if items.is_empty() {
                            items.push(Item::line_swallowing(Doc::text(t.text()), blank));
                        } else {
                            // A `@<` post-annotation is separated from the item it
                            // documents by two spaces (before alignment padding).
                            attach(
                                &mut items,
                                Doc::anchor(AnchorKind::PostAnnotation, Doc::text(t.text())),
                                "  ",
                            );
                        }
                        saw_eol = false;
                        blank = false;
                        newlines = 0;
                        started = true;
                    }
                    _ => {
                        items.push(Item::new(Doc::text(t.text()), blank));
                        saw_eol = false;
                        blank = false;
                        newlines = 0;
                        started = true;
                    }
                }
            } else if let Some(n) = child.as_node() {
                items.push(Item::new(self.lower(n), blank));
                saw_eol = false;
                blank = false;
                newlines = 0;
                started = true;
            }
        }
        Collected {
            items,
            trailing_blank,
        }
    }

    // ---- dispatch ------------------------------------------------------

    fn lower(&self, node: &SyntaxNode) -> Doc {
        let k = node.kind();
        if k.is_member_list() {
            return self.lower_member_list(node);
        }
        match k {
            DEF_CHOICE => self.lower_choice(node),
            _ if is_explodable(k) => self.lower_spec(node),
            _ => self.lower_inline(node),
        }
    }

    /// Concatenate a node's children on one logical line using tight spacing
    /// rules; child member lists still render as blocks/groups.
    fn lower_inline(&self, node: &SyntaxNode) -> Doc {
        let mut parts: Vec<Doc> = Vec::new();
        let mut prev: Option<SyntaxToken> = None;
        for child in node.children_with_tokens() {
            if let Some(t) = child.as_token() {
                if is_trivia(t.kind()) {
                    continue;
                }
                if let Some(p) = &prev {
                    parts.push(sep_doc(p, t));
                }
                parts.push(self.lower_token(node, t));
                prev = Some(t.clone());
            } else if let Some(n) = child.as_node() {
                if let (Some(p), Some(f)) = (&prev, first_token(n)) {
                    parts.push(sep_doc(p, &f));
                }
                parts.push(self.lower(n));
                if let Some(l) = last_token(n) {
                    prev = Some(l);
                }
            }
        }
        Doc::concat(parts)
    }

    fn lower_token(&self, parent: &SyntaxNode, t: &SyntaxToken) -> Doc {
        if t.kind() == RIGHT_ARROW && parent.kind() == CONNECTION {
            Doc::anchor(AnchorKind::Arrow, Doc::text(t.text()))
        } else if t.kind() == EQUALS && matches!(parent.kind(), DEF_CONSTANT | DEF_ENUM_CONSTANT) {
            Doc::anchor(AnchorKind::Equals, Doc::text(t.text()))
        } else {
            Doc::text(t.text())
        }
    }

    // ---- member lists --------------------------------------------------

    fn lower_member_list(&self, node: &SyntaxNode) -> Doc {
        let cfg = list_cfg(node.kind());
        let Collected {
            items,
            trailing_blank,
        } = self.collect_items(node);
        let always = match cfg.mode {
            ListMode::Always => true,
            ListMode::Auto => false,
        };
        let force_block = always || items.iter().any(|i| i.comment);

        // A definition-scope body preserves a single blank line at its start
        // and end when the source had one; other block bodies (structs, enums,
        // sub-blocks, inline lists) always hug their braces.
        let keep_blank = preserves_edge_blanks(node.kind());

        if items.is_empty() {
            return match cfg.mode {
                // An empty scope keeps its lone blank line if the source had
                // one (it is simultaneously the body's start and end).
                ListMode::Always => Doc::concat(vec![
                    Doc::text(cfg.open),
                    break_lines(keep_blank && trailing_blank),
                    Doc::text(cfg.close),
                ]),
                ListMode::Auto => Doc::text(format!("{}{}", cfg.open, cfg.close)),
            };
        }
        let lead_blank = keep_blank && items.first().is_some_and(|i| i.blank_before);
        let trail_blank = keep_blank && trailing_blank;

        let pad = if cfg.open == "{" { " " } else { "" };
        let (open_line, body) = if force_block {
            (break_lines(lead_blank), join_items(&items))
        } else {
            (
                Doc::Line {
                    flat: pad,
                    cont: false,
                },
                join_items_sep(
                    &items,
                    Doc::Line {
                        flat: ", ",
                        cont: false,
                    },
                ),
            )
        };

        let mut inner = vec![
            Doc::text(cfg.open),
            Doc::nest(1, Doc::concat(vec![open_line, body])),
        ];
        match cfg.close_style {
            Close::Dedent => {
                inner.push(if force_block {
                    break_lines(trail_blank)
                } else {
                    Doc::Line {
                        flat: pad,
                        cont: false,
                    }
                });
                inner.push(Doc::text(cfg.close));
            }
            Close::Trail => {
                inner.push(if force_block {
                    break_lines(trail_blank)
                } else {
                    Doc::Line {
                        flat: "",
                        cont: false,
                    }
                });
                inner.push(Doc::text(cfg.close));
            }
        }

        let doc = Doc::concat(inner);
        if force_block { doc } else { Doc::group(doc) }
    }

    // ---- explodable specifiers -----------------------------------------

    fn lower_spec(&self, node: &SyntaxNode) -> Doc {
        let (head, clauses) = self.segment_clauses(node);
        if clauses.is_empty() {
            return head;
        }
        let mut parts = vec![head];
        for c in clauses {
            parts.push(Doc::nest(1, Doc::concat(vec![Doc::contline(), c])));
        }
        Doc::group(Doc::concat(parts))
    }

    /// Split a spec node into a head Doc and per-clause Docs.
    fn segment_clauses(&self, node: &SyntaxNode) -> (Doc, Vec<Doc>) {
        let parent = node.kind();
        let mut head: Vec<Doc> = Vec::new();
        let mut clauses: Vec<Vec<Doc>> = Vec::new();
        let mut prev: Option<SyntaxToken> = None;
        let mut in_clauses = false;

        for child in node.children_with_tokens() {
            let (ft, doc, lt, starts) = if let Some(t) = child.as_token() {
                if is_trivia(t.kind()) {
                    continue;
                }
                (
                    Some(t.clone()),
                    self.lower_token(node, t),
                    Some(t.clone()),
                    is_clause_keyword(parent, t.kind()),
                )
            } else if let Some(n) = child.as_node() {
                (
                    first_token(n),
                    self.lower(n),
                    last_token(n),
                    is_clause_node(n.kind()),
                )
            } else {
                continue;
            };

            if starts {
                in_clauses = true;
                clauses.push(Vec::new());
            }

            let sink = if in_clauses {
                clauses.last_mut().unwrap()
            } else {
                &mut head
            };
            if !sink.is_empty()
                && let (Some(p), Some(f)) = (&prev, &ft)
            {
                sink.push(sep_doc(p, f));
            }
            sink.push(doc);
            if lt.is_some() {
                prev = lt;
            }
        }

        (
            Doc::concat(head),
            clauses.into_iter().map(Doc::concat).collect(),
        )
    }

    // ---- choice --------------------------------------------------------

    fn lower_choice(&self, node: &SyntaxNode) -> Doc {
        let mut head: Vec<Doc> = Vec::new();
        let mut body: Vec<Doc> = Vec::new();
        let mut prev: Option<SyntaxToken> = None;
        let mut in_body = false;

        for child in node.children_with_tokens() {
            if let Some(t) = child.as_token() {
                if is_trivia(t.kind()) {
                    continue;
                }
                match t.kind() {
                    LEFT_CURLY => {
                        in_body = true;
                        prev = None;
                        continue;
                    }
                    RIGHT_CURLY => continue,
                    _ => {}
                }
                let sink = if in_body { &mut body } else { &mut head };
                if let Some(p) = &prev {
                    sink.push(sep_doc(p, t));
                }
                sink.push(Doc::text(t.text()));
                prev = Some(t.clone());
            } else if let Some(n) = child.as_node() {
                let f = first_token(n);
                let sink = if in_body { &mut body } else { &mut head };
                if let (Some(p), Some(f)) = (&prev, &f) {
                    sink.push(sep_doc(p, f));
                }
                sink.push(self.lower(n));
                if let Some(l) = last_token(n) {
                    prev = Some(l);
                }
            }
        }

        Doc::concat(vec![
            Doc::concat(head),
            Doc::text(" {"),
            Doc::nest(
                1,
                Doc::concat(vec![Doc::hardline(), Doc::group(Doc::concat(body))]),
            ),
            Doc::hardline(),
            Doc::text("}"),
        ])
    }
}

// ============================================================================
// Items
// ============================================================================

/// The members of a container plus whether a blank line preceded its close.
struct Collected {
    items: Vec<Item>,
    trailing_blank: bool,
}

/// A hard break, doubled to `\n\n` (an intentional blank line) when `blank`.
fn break_lines(blank: bool) -> Doc {
    if blank {
        Doc::concat(vec![Doc::hardline(), Doc::hardline()])
    } else {
        Doc::hardline()
    }
}

struct Item {
    doc: Doc,
    blank_before: bool,
    comment: bool,
}

impl Item {
    fn new(doc: Doc, blank_before: bool) -> Self {
        Item {
            doc,
            blank_before,
            comment: false,
        }
    }

    /// An item whose text swallows the rest of its source line (a comment or an
    /// annotation). Such items can never share a flat line with a sibling, so
    /// they force their enclosing list to render as a block.
    fn line_swallowing(doc: Doc, blank_before: bool) -> Self {
        Item {
            doc,
            blank_before,
            comment: true,
        }
    }
}

/// Append `extra` to the preceding item on the same line, separated by `gap`
/// (a trailing comment uses one space, a `@<` annotation two). If that item is
/// itself line-swallowing, `extra` drops to the next line instead.
fn attach(items: &mut [Item], extra: Doc, gap: &str) {
    if let Some(last) = items.last_mut() {
        let sep = if last.comment {
            Doc::hardline()
        } else {
            Doc::text(gap)
        };
        let d = std::mem::replace(&mut last.doc, Doc::Nil);
        last.doc = Doc::concat(vec![d, sep, extra]);
        last.comment = true;
    }
}

fn join_items(items: &[Item]) -> Doc {
    let mut parts = Vec::new();
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            parts.push(Doc::hardline());
            if it.blank_before {
                parts.push(Doc::hardline());
            }
        }
        parts.push(it.doc.clone());
    }
    Doc::concat(parts)
}

fn join_items_sep(items: &[Item], sep: Doc) -> Doc {
    let mut parts = Vec::new();
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            parts.push(sep.clone());
        }
        parts.push(it.doc.clone());
    }
    Doc::concat(parts)
}

// ============================================================================
// Classification
// ============================================================================

fn is_explodable(kind: SyntaxKind) -> bool {
    kind.is_spec() || kind == DEF_COMPONENT_INSTANCE
}

/// Whether a block body preserves a single blank line at its start and end when
/// the source had one. True for definition scopes and connection blocks; other
/// bodies (structs, enums, packet sub-blocks, inline lists) hug their
/// delimiters.
fn preserves_edge_blanks(kind: SyntaxKind) -> bool {
    match kind {
        MODULE_MEMBER_LIST
        | COMPONENT_MEMBER_LIST
        | INTERFACE_MEMBER_LIST
        | TOPOLOGY_MEMBER_LIST
        | STATE_MACHINE_MEMBER_LIST
        | STATE_MEMBER_LIST
        | CONNECTION_MEMBER_LIST => true,
        _ => false,
    }
}

fn is_clause_node(kind: SyntaxKind) -> bool {
    match kind {
        OPCODE
        | PRIORITY
        | QUEUE_FULL
        | ID
        | FORMAT
        | EVENT_THROTTLE
        | EVENT_SEVERITY
        | DEFAULT
        | QUEUE_SIZE
        | STACK_SIZE
        | CPU
        | COMPONENT_INSTANCE_TYPE
        | COMPONENT_INSTANCE_FILE => true,
        _ => false,
    }
}

/// Loose keyword clause-starters, by enclosing spec kind.
fn is_clause_keyword(parent: SyntaxKind, kind: SyntaxKind) -> bool {
    match (parent, kind) {
        (SPEC_TELEMETRY, UPDATE_KW | LOW_KW | HIGH_KW)
        | (SPEC_PARAM, SET_KW | SAVE_KW)
        | (SPEC_CONTAINER, DEFAULT_KW)
        | (SPEC_RECORD, ARRAY_KW) => true,
        _ => false,
    }
}

// ---- member-list configuration --------------------------------------------

enum ListMode {
    Always,
    Auto,
}

enum Close {
    Dedent,
    Trail,
}

struct ListCfg {
    open: &'static str,
    close: &'static str,
    mode: ListMode,
    close_style: Close,
}

fn list_cfg(kind: SyntaxKind) -> ListCfg {
    match kind {
        FORMAL_PARAM_LIST => ListCfg {
            open: "(",
            close: ")",
            mode: ListMode::Auto,
            close_style: Close::Trail,
        },
        EXPR_ARRAY_MEMBER_LIST => ListCfg {
            open: "[",
            close: "]",
            mode: ListMode::Auto,
            close_style: Close::Trail,
        },
        EXPR_STRUCT_MEMBER_LIST | DO_EXPR_MEMBER_LIST => ListCfg {
            open: "{",
            close: "}",
            mode: ListMode::Auto,
            close_style: Close::Dedent,
        },
        _ => ListCfg {
            open: "{",
            close: "}",
            mode: ListMode::Always,
            close_style: Close::Dedent,
        },
    }
}

// ============================================================================
// Token helpers
// ============================================================================

fn is_trivia(kind: SyntaxKind) -> bool {
    match kind {
        WHITESPACE | EOL => true,
        _ => false,
    }
}

fn first_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| !is_trivia(t.kind()))
}

fn last_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !is_trivia(t.kind()))
        .last()
}

/// Inter-token separator as a `Doc`. A line comment or annotation swallows the
/// rest of its source line, so anything after one must start on a new line;
/// otherwise fall back to the flat spacing rules.
fn sep_doc(prev: &SyntaxToken, cur: &SyntaxToken) -> Doc {
    let after_comment = match prev.kind() {
        COMMENT | PRE_ANNOTATION | POST_ANNOTATION => true,
        _ => false,
    };
    if after_comment {
        Doc::hardline()
    } else {
        Doc::text(flat_sep(prev, cur))
    }
}

/// Flat inter-token separator (" " or "").
fn flat_sep(prev: &SyntaxToken, cur: &SyntaxToken) -> &'static str {
    // A `@<` post-annotation is always preceded by two spaces, matching the
    // member-list path in `attach`.
    if cur.kind() == POST_ANNOTATION {
        return "  ";
    }
    if prev.kind() == DOT || cur.kind() == DOT {
        return "";
    }
    // The enum representation-type colon (`enum E : U8`) takes a leading space,
    // unlike the tight `name: Type` colon of struct fields and formal params.
    if cur.kind() == COLON && cur.parent().is_some_and(|p| p.kind() == DEF_ENUM) {
        return " ";
    }
    let cur_tight = match cur.kind() {
        COLON | COMMA | SEMI | RIGHT_PAREN | RIGHT_SQUARE => true,
        _ => false,
    };
    if cur_tight {
        return "";
    }
    if cur.kind() == LEFT_SQUARE && is_subscript_bracket(cur) {
        return "";
    }
    let prev_open = match prev.kind() {
        LEFT_PAREN | LEFT_SQUARE => true,
        _ => false,
    };
    if prev_open {
        return "";
    }
    let prev_sign = match prev.kind() {
        MINUS | PLUS => true,
        _ => false,
    };
    if prev_sign && is_unary(prev) {
        return "";
    }
    let prev_callable = match prev.kind() {
        IDENT | RIGHT_PAREN | RIGHT_SQUARE | SIZEOF_KW => true,
        _ => false,
    };
    if cur.kind() == LEFT_PAREN && prev_callable {
        return "";
    }
    " "
}

fn is_unary(tok: &SyntaxToken) -> bool {
    tok.parent().is_some_and(|p| p.kind() == EXPR_UNARY)
}

fn is_subscript_bracket(tok: &SyntaxToken) -> bool {
    let Some(ios) = tok.parent() else {
        return false;
    };
    if ios.kind() != INDEX_OR_SIZE {
        return false;
    }
    ios.parent().is_some_and(|gp| match gp.kind() {
        EXPR_SUBSCRIPT | EXPR_POSTFIX | CONNECTION_FROM | CONNECTION_TO => true,
        _ => false,
    })
}
