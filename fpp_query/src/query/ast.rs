//! Syntax tree of the query language, plus the static summary of what a query
//! needs materialized before it can be evaluated.

use fpp_core::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Contains,
    StartsWith,
    EndsWith,
    Matches,
}

impl RelOp {
    pub fn spelling(self) -> &'static str {
        match self {
            RelOp::Eq => "==",
            RelOp::Ne => "!=",
            RelOp::Lt => "<",
            RelOp::Le => "<=",
            RelOp::Gt => ">",
            RelOp::Ge => ">=",
            RelOp::Contains => "contains",
            RelOp::StartsWith => "starts_with",
            RelOp::EndsWith => "ends_with",
            RelOp::Matches => "matches",
        }
    }

    /// True for the operators that hold when they hold for at least one element
    /// of a list on the left. `$@ contains "x"` reads over annotation lines this
    /// way; `==` does not, so `$.members == 3` stays an error.
    pub fn lifts_over_lists(self) -> bool {
        matches!(
            self,
            RelOp::Contains | RelOp::StartsWith | RelOp::EndsWith | RelOp::Matches
        )
    }
}

/// A `$@…` root: data that lives in the compiler context or in the walk, not in
/// the node's own fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meta {
    /// Every annotation line, pre then post.
    All,
    Pre,
    Post,
    /// The node kind name, e.g. `"DefTopology"`.
    Kind,
    /// URI of the file the node was parsed from.
    File,
    /// 1-based start line.
    Line,
    /// True when the node was spliced in by an `include` specifier.
    Included,
    /// Dotted enclosing scope, `""` at top level.
    Scope,
    /// Dotted enclosing scope and the node's own name.
    Qualified,
    /// The default filename stem for this node.
    Stem,
}

impl Meta {
    pub const ALL: &'static [(&'static str, Meta)] = &[
        ("pre", Meta::Pre),
        ("post", Meta::Post),
        ("kind", Meta::Kind),
        ("file", Meta::File),
        ("line", Meta::Line),
        ("included", Meta::Included),
        ("scope", Meta::Scope),
        ("qualified", Meta::Qualified),
        ("stem", Meta::Stem),
    ];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Root {
    /// `$` — the matched node.
    Node,
    /// `$@...`
    Meta(Meta),
    /// `$^` (immediate parent) or `$^DefComponent` (nearest enclosing of kind).
    Ancestor(Option<String>),
}

#[derive(Debug, Clone)]
pub enum Seg {
    Field(String, Span),
    Index(usize, Span),
}

impl Seg {
    pub fn span(&self) -> Span {
        match self {
            Seg::Field(_, s) | Seg::Index(_, s) => *s,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Path {
    pub root: Root,
    pub segs: Vec<Seg>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Str(String, Span),
    Int(i64, Span),
    Bool(bool, Span),
    Null(Span),
    Path(Path),
    Call {
        name: String,
        args: Vec<Expr>,
        span: Span,
    },
    Not(Box<Expr>, Span),
    And(Box<Expr>, Box<Expr>, Span),
    Or(Box<Expr>, Box<Expr>, Span),
    Rel {
        op: RelOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    /// `x in <list>`. The right side is any list-valued expression, so both a
    /// literal set (`$.kind in ["Active", "Queued"]`) and a list read off the node
    /// (`"static-tlm-packetizer" in $@`) are the same construct.
    In {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    /// A bracketed list literal.
    List(Vec<Expr>, Span),
    Cat(Box<Expr>, Box<Expr>, Span),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Str(_, s)
            | Expr::Int(_, s)
            | Expr::Bool(_, s)
            | Expr::Null(s)
            | Expr::Call { span: s, .. }
            | Expr::Not(_, s)
            | Expr::And(_, _, s)
            | Expr::Or(_, _, s)
            | Expr::Rel { span: s, .. }
            | Expr::In { span: s, .. }
            | Expr::List(_, s)
            | Expr::Cat(_, _, s) => *s,
            Expr::Path(p) => p.span,
        }
    }
}

/// What a query touches, computed once at parse time: whether it reads the
/// matched node, and which `$^…` ancestors it reads.
#[derive(Debug, Clone, Default)]
pub struct Needs {
    /// The query reads a `$`-rooted path.
    pub node: bool,
    /// One entry per distinct `$^...` root: `None` for `$^`, `Some(kind)` for
    /// `$^Kind`.
    pub ancestors: Vec<Option<String>>,
}

impl Needs {
    pub fn of(expr: &Expr) -> Needs {
        let mut needs = Needs::default();
        needs.visit(expr);
        needs
    }

    /// Union of what several expressions need, so one walk covers a group's
    /// `--where` and `--name` together.
    pub fn union(exprs: impl IntoIterator<Item = Needs>) -> Needs {
        let mut out = Needs::default();
        for n in exprs {
            out.node |= n.node;
            for a in n.ancestors {
                if !out.ancestors.contains(&a) {
                    out.ancestors.push(a);
                }
            }
        }
        out
    }

    fn visit(&mut self, expr: &Expr) {
        match expr {
            Expr::Str(..) | Expr::Int(..) | Expr::Bool(..) | Expr::Null(_) => {}
            Expr::Path(p) => match &p.root {
                Root::Node => self.node = true,
                Root::Meta(_) => {}
                Root::Ancestor(kind) => {
                    if !self.ancestors.contains(kind) {
                        self.ancestors.push(kind.clone());
                    }
                }
            },
            Expr::Not(e, _) => self.visit(e),
            Expr::And(a, b, _) | Expr::Or(a, b, _) | Expr::Cat(a, b, _) => {
                self.visit(a);
                self.visit(b);
            }
            Expr::Rel { lhs, rhs, .. } | Expr::In { lhs, rhs, .. } => {
                self.visit(lhs);
                self.visit(rhs);
            }
            Expr::List(items, _) => items.iter().for_each(|i| self.visit(i)),
            Expr::Call { args, .. } => args.iter().for_each(|a| self.visit(a)),
        }
    }
}
