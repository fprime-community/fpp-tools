//! Recursive-descent parser for the query language.

use super::ast::{Expr, Meta, Path, RelOp, Root, Seg};
use super::lex::{Lexed, Tok, lex};
use crate::Diagnosed;
use fpp_core::{SourceFile, Span};

pub struct Function {
    pub name: &'static str,
    pub arity: usize,
    pub help: &'static str,
}

pub const FUNCTIONS: &[Function] = &[
    Function {
        name: "len",
        arity: 1,
        help: "number of elements in a list, or characters in a string",
    },
    Function {
        name: "lower",
        arity: 1,
        help: "ASCII-lowercase a string",
    },
    Function {
        name: "upper",
        arity: 1,
        help: "ASCII-uppercase a string",
    },
    Function {
        name: "join",
        arity: 2,
        help: "join a list with a separator: join($.implements, \", \")",
    },
    Function {
        name: "replace",
        arity: 3,
        help: "replace every occurrence: replace($@qualified, \".\", \"_\")",
    },
];

/// Words that are operators rather than identifiers.
fn word_op(word: &str) -> Option<RelOp> {
    match word {
        "contains" => Some(RelOp::Contains),
        "starts_with" => Some(RelOp::StartsWith),
        "ends_with" => Some(RelOp::EndsWith),
        "matches" => Some(RelOp::Matches),
        _ => None,
    }
}

/// True for every word an operator can be spelled with. Whitespace is not a
/// token, so `$@ contains "x"` and `$@contains "x"` reach the parser identically.
fn is_operator_word(word: &str) -> bool {
    word_op(word).is_some() || word == "in"
}

/// The operator of the relational level. `in` is written like the others but
/// builds a different node, so it is a variant here rather than a [`RelOp`].
#[derive(Clone, Copy)]
enum Rel {
    Op(RelOp),
    In,
}

impl Rel {
    fn spelling(self) -> &'static str {
        match self {
            Rel::Op(op) => op.spelling(),
            Rel::In => "in",
        }
    }
}

/// Both bounds exist so a pathological input produces a diagnostic instead of a
/// stack overflow: [`MAX_TOKENS`] caps the input, [`MAX_DEPTH`] the nesting.
const MAX_TOKENS: usize = 4096;
const MAX_DEPTH: usize = 64;

pub struct Parser {
    lexed: Lexed,
    at: usize,
    /// Nesting depth of the productions that recurse.
    depth: usize,
}

/// Parse `text` into an expression. `text` begins at byte `offset` of `file`, so
/// a query written inside a rules file reports against that file's real lines.
pub fn parse(file: SourceFile, offset: usize, text: &str) -> Result<Expr, Diagnosed> {
    let lexed = lex(file, offset, text)?;
    if lexed.toks.len() > MAX_TOKENS {
        return Err(error(
            lexed.spans[0],
            format!(
                "query is too long: {} tokens, limit {MAX_TOKENS}",
                lexed.toks.len()
            ),
        ));
    }

    let mut parser = Parser {
        lexed,
        at: 0,
        depth: 0,
    };

    let expr = parser.expr()?;
    if parser.peek(0) != &Tok::Eof {
        return Err(parser.err(format!(
            "unexpected {} after the query",
            parser.peek(0).describe()
        )));
    }

    Ok(expr)
}

/// Emit an error against `span` and return the marker that ends the parse.
fn error(span: Span, message: String) -> Diagnosed {
    span.error(message).emit();
    Diagnosed
}

/// A word in value position that is neither a literal nor a call.
fn err_unknown_name(span: Span, word: &str) -> Diagnosed {
    let diag = span.error(format!("unknown name `{word}` in query"));
    if is_operator_word(word) {
        diag.note(format!(
            "`{word}` is an operator; it needs a value on each side"
        ))
    } else {
        diag.note("a field of the matched node is written `$.name`")
            .note(format!("available functions: {}", function_list()))
    }
    .emit();
    Diagnosed
}

fn err_unknown_function(span: Span, name: &str) -> Diagnosed {
    span.error(format!("unknown function `{name}`"))
        .note(format!("available functions: {}", function_list()))
        .emit();
    Diagnosed
}

fn err_unknown_meta(span: Span, word: &str) -> Diagnosed {
    span.error(format!("unknown metadata `$@{word}`"))
        .note(format!(
            "available: $@ (all annotation lines), {}",
            meta_list()
        ))
        .emit();
    Diagnosed
}

impl Parser {
    fn peek(&self, ahead: usize) -> &Tok {
        self.lexed
            .toks
            .get(self.at + ahead)
            .unwrap_or(&self.lexed.toks[self.lexed.toks.len() - 1])
    }

    /// The word at the cursor, when it names something rather than being one of
    /// the operators spelled with a word. A path root reads this: `$@ contains
    /// "x"` reaches the parser as `$@` followed by `contains`, which has to be
    /// the operator and not the name of a metadatum.
    fn peek_name(&self) -> Option<String> {
        match self.peek(0) {
            Tok::Ident(word) if !is_operator_word(word) => Some(word.clone()),
            _ => None,
        }
    }

    /// Span of the token at the cursor.
    fn span(&self) -> Span {
        self.lexed.spans[self.at]
    }

    /// A span covering everything from `first` through the token just consumed.
    fn span_from(&self, first: Span) -> Span {
        let end = self.lexed.spans[self.at.saturating_sub(1)];
        let (start, end) = (first.start().pos(), end.end().pos());
        Span::new(first.file(), start, end.saturating_sub(start), None)
    }

    fn next(&mut self) -> Tok {
        let tok = self.lexed.toks[self.at].clone();
        if tok != Tok::Eof {
            self.at += 1;
        }
        tok
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek(0) == tok {
            self.next();
            true
        } else {
            false
        }
    }

    fn consume(&mut self, tok: &Tok) -> Result<(), Diagnosed> {
        if self.eat(tok) {
            return Ok(());
        }
        Err(self.err(format!(
            "expected {}, found {}",
            tok.describe(),
            self.peek(0).describe()
        )))
    }

    /// Emit an error against the token at the cursor.
    fn err(&self, message: String) -> Diagnosed {
        error(self.span(), message)
    }

    /// Enter a nesting level, refusing to go deeper than [`MAX_DEPTH`]. Nesting
    /// only deepens through [`Parser::expr`] — every parenthesis, list element
    /// and call argument recurses through it — and through the `!` chain, so
    /// those are the two productions that count.
    fn deeper(&mut self) -> Result<(), Diagnosed> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err(format!("query nests deeper than {MAX_DEPTH} levels")));
        }
        Ok(())
    }

    fn shallower(&mut self) {
        self.depth -= 1;
    }

    fn expr(&mut self) -> Result<Expr, Diagnosed> {
        self.deeper()?;
        let first = self.span();
        let mut left = self.expr_or_operand()?;

        while self.eat(&Tok::OrOr) {
            let right = self.expr_or_operand()?;
            left = Expr::Or(Box::new(left), Box::new(right), self.span_from(first));
        }

        self.shallower();
        Ok(left)
    }

    fn expr_or_operand(&mut self) -> Result<Expr, Diagnosed> {
        let first = self.span();
        let mut left = self.expr_and_operand()?;

        while self.eat(&Tok::AndAnd) {
            let right = self.expr_and_operand()?;
            left = Expr::And(Box::new(left), Box::new(right), self.span_from(first));
        }

        Ok(left)
    }

    fn expr_and_operand(&mut self) -> Result<Expr, Diagnosed> {
        let first = self.span();
        if !self.eat(&Tok::Bang) {
            return self.expr_not_operand();
        }

        self.deeper()?;
        let inner = self.expr_and_operand()?;
        self.shallower();

        Ok(Expr::Not(Box::new(inner), self.span_from(first)))
    }

    /// The relational level, which is non-associative: `a == b == c` is a mistake
    /// rather than a chained comparison, so a second operator here is an error.
    fn expr_not_operand(&mut self) -> Result<Expr, Diagnosed> {
        let first = self.span();
        let left = self.expr_rel_operand()?;

        let Some(rel) = self.rel() else {
            return Ok(left);
        };
        self.next();
        let right = self.expr_rel_operand()?;
        let span = self.span_from(first);

        if self.rel().is_some() {
            return Err(self.err(format!(
                "`{}` cannot be chained; parenthesize one side",
                rel.spelling()
            )));
        }

        Ok(match rel {
            Rel::Op(op) => Expr::Rel {
                op,
                lhs: Box::new(left),
                rhs: Box::new(right),
                span,
            },
            Rel::In => Expr::In {
                lhs: Box::new(left),
                rhs: Box::new(right),
                span,
            },
        })
    }

    /// The relational operator at the cursor, if the cursor is on one.
    fn rel(&self) -> Option<Rel> {
        match self.peek(0) {
            Tok::EqEq => Some(Rel::Op(RelOp::Eq)),
            Tok::BangEq => Some(Rel::Op(RelOp::Ne)),
            Tok::Lt => Some(Rel::Op(RelOp::Lt)),
            Tok::Le => Some(Rel::Op(RelOp::Le)),
            Tok::Gt => Some(Rel::Op(RelOp::Gt)),
            Tok::Ge => Some(Rel::Op(RelOp::Ge)),
            Tok::Ident(word) if word == "in" => Some(Rel::In),
            Tok::Ident(word) => word_op(word).map(Rel::Op),
            _ => None,
        }
    }

    fn expr_rel_operand(&mut self) -> Result<Expr, Diagnosed> {
        let first = self.span();
        let mut left = self.expr_primary()?;

        while self.eat(&Tok::Plus) {
            let right = self.expr_primary()?;
            left = Expr::Cat(Box::new(left), Box::new(right), self.span_from(first));
        }

        Ok(left)
    }

    fn expr_primary(&mut self) -> Result<Expr, Diagnosed> {
        let first = self.span();
        match self.peek(0).clone() {
            Tok::LParen => {
                self.next();
                let inner = self.expr()?;
                self.consume(&Tok::RParen)?;
                Ok(inner)
            }
            Tok::LBracket => self.list(),
            Tok::Dollar => self.path().map(Expr::Path),
            Tok::Str(s) => {
                self.next();
                Ok(Expr::Str(s, first))
            }
            Tok::Int(n) => {
                self.next();
                Ok(Expr::Int(n, first))
            }
            Tok::Ident(word) => self.expr_ident(word),
            other => Err(self.err(format!("expected a value, found {}", other.describe()))),
        }
    }

    /// A primary that begins with a word: a literal, a call, or a mistake. The
    /// cursor is on `word`.
    fn expr_ident(&mut self, word: String) -> Result<Expr, Diagnosed> {
        let first = self.span();
        match word.as_str() {
            "true" => {
                self.next();
                Ok(Expr::Bool(true, first))
            }
            "false" => {
                self.next();
                Ok(Expr::Bool(false, first))
            }
            "null" => {
                self.next();
                Ok(Expr::Null(first))
            }
            _ if self.peek(1) == &Tok::LParen => self.call(word),
            _ => Err(err_unknown_name(first, &word)),
        }
    }

    fn list(&mut self) -> Result<Expr, Diagnosed> {
        let first = self.span();
        self.consume(&Tok::LBracket)?;
        let items = self.elements(&Tok::RBracket)?;
        self.consume(&Tok::RBracket)?;

        Ok(Expr::List(items, self.span_from(first)))
    }

    /// A call to one of [`FUNCTIONS`]. The cursor is on `name`.
    fn call(&mut self, name: String) -> Result<Expr, Diagnosed> {
        let first = self.span();
        let Some(f) = FUNCTIONS.iter().find(|f| f.name == name) else {
            return Err(err_unknown_function(first, &name));
        };

        self.next();
        self.consume(&Tok::LParen)?;
        let args = self.elements(&Tok::RParen)?;
        self.consume(&Tok::RParen)?;

        let span = self.span_from(first);
        if args.len() != f.arity {
            return Err(error(
                span,
                format!(
                    "`{name}` takes {} argument{}, found {}",
                    f.arity,
                    if f.arity == 1 { "" } else { "s" },
                    args.len()
                ),
            ));
        }

        Ok(Expr::Call { name, args, span })
    }

    /// A possibly empty comma-separated element list, up to but not including
    /// `end`. Shared by the list literal and a call's arguments.
    fn elements(&mut self, end: &Tok) -> Result<Vec<Expr>, Diagnosed> {
        let mut out = Vec::new();
        if self.peek(0) != end {
            loop {
                out.push(self.expr()?);
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
        }

        Ok(out)
    }

    fn path(&mut self) -> Result<Path, Diagnosed> {
        let first = self.span();
        self.consume(&Tok::Dollar)?;
        let root = self.path_root()?;
        let segs = self.path_segs()?;

        Ok(Path {
            root,
            segs,
            span: self.span_from(first),
        })
    }

    /// What a path reads from: the matched node, a `$@…` metadatum, or a `$^…`
    /// ancestor. The `$` has been consumed.
    fn path_root(&mut self) -> Result<Root, Diagnosed> {
        if self.eat(&Tok::At) {
            self.meta_root()
        } else if self.eat(&Tok::Caret) {
            self.ancestor_root()
        } else {
            Ok(Root::Node)
        }
    }

    /// The metadatum named after `$@`. Bare `$@` is every annotation line, pre
    /// then post.
    fn meta_root(&mut self) -> Result<Root, Diagnosed> {
        let Some(word) = self.peek_name() else {
            return Ok(Root::Meta(Meta::All));
        };
        let span = self.span();
        self.next();

        let Some(&(_, meta)) = Meta::ALL.iter().find(|(n, _)| *n == word) else {
            return Err(err_unknown_meta(span, &word));
        };

        Ok(Root::Meta(meta))
    }

    /// The node kind named after `$^`. Bare `$^` is the immediate parent.
    fn ancestor_root(&mut self) -> Result<Root, Diagnosed> {
        let Some(kind) = self.peek_name() else {
            return Ok(Root::Ancestor(None));
        };
        let span = self.span();
        self.next();

        if !fpp_ast::Node::KIND_NAMES.contains(&kind.as_str()) {
            crate::naming::unknown_kind(span, &kind).emit();
            return Err(Diagnosed);
        }

        Ok(Root::Ancestor(Some(kind)))
    }

    fn path_segs(&mut self) -> Result<Vec<Seg>, Diagnosed> {
        let mut segs = Vec::new();
        loop {
            match self.peek(0) {
                Tok::Dot => segs.push(self.field_seg()?),
                Tok::LBracket => segs.push(self.index_seg()?),
                _ => return Ok(segs),
            }
        }
    }

    fn field_seg(&mut self) -> Result<Seg, Diagnosed> {
        self.consume(&Tok::Dot)?;
        let span = self.span();
        match self.next() {
            Tok::Ident(name) => Ok(Seg::Field(name, span)),
            other => Err(error(
                span,
                format!("expected a field name, found {}", other.describe()),
            )),
        }
    }

    fn index_seg(&mut self) -> Result<Seg, Diagnosed> {
        let first = self.span();
        self.consume(&Tok::LBracket)?;

        let span = self.span();
        let index = match self.next() {
            Tok::Int(n) if n >= 0 => usize::try_from(n).unwrap_or(usize::MAX),
            Tok::Int(_) => return Err(error(span, "list index cannot be negative".to_string())),
            other => {
                return Err(error(
                    span,
                    format!("expected a list index, found {}", other.describe()),
                ));
            }
        };
        self.consume(&Tok::RBracket)?;

        Ok(Seg::Index(index, self.span_from(first)))
    }
}

fn function_list() -> String {
    FUNCTIONS
        .iter()
        .map(|f| f.name)
        .collect::<Vec<_>>()
        .join(", ")
}

fn meta_list() -> String {
    Meta::ALL
        .iter()
        .map(|(n, _)| format!("$@{n}"))
        .collect::<Vec<_>>()
        .join(", ")
}
