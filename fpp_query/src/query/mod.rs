//! The query language: a small expression language over one AST node.
//!
//! A query is evaluated against `serde_json::Value`s produced by `fpp_ast`'s
//! `Serialize` impls. Its grammar, in full:
//!
//! ```text
//! query            = expr
//! expr             = expr_or_operand { "||" expr_or_operand }
//! expr_or_operand  = expr_and_operand { "&&" expr_and_operand }
//! expr_and_operand = "!" expr_and_operand | expr_not_operand
//! expr_not_operand = expr_rel_operand [ rel expr_rel_operand ]
//! rel              = "==" | "!=" | "<" | "<=" | ">" | ">=" | "in"
//!                  | "contains" | "starts_with" | "ends_with" | "matches"
//! expr_rel_operand = expr_primary { "+" expr_primary }
//! expr_primary     = literal | path | call | list | "(" expr ")"
//! list             = "[" [ elements ] "]"
//! call             = ident "(" [ elements ] ")"
//! elements         = expr { "," expr }
//! path             = path_root { path_seg }
//! path_root        = "$" [ "@" [ ident ] | "^" [ Kind ] ]
//! path_seg         = "." ident | "[" int "]"
//! literal          = string | int | "true" | "false" | "null"
//! ```
//!
//! Every production is one function of the same name in `parse`. Relational
//! operators are non-associative, so `a == b == c` is a parse error rather than a
//! surprise.

use crate::Diagnosed;
use fpp_core::SourceFile;

pub mod ast;
pub mod eval;
pub(crate) mod lex;
mod parse;

pub use ast::{Expr, Meta, Needs};
pub use eval::Bindings;
pub use parse::FUNCTIONS;

/// A parsed query, together with what it needs materialized to be evaluated.
pub struct Query {
    pub expr: Expr,
    pub needs: Needs,
}

/// Parse `text`, which begins at byte `offset` of `file`. A query is always a
/// substring of the rules file it is written in, so its diagnostics caret the
/// offending sub-expression where it is actually written.
///
/// Must be called inside a `fpp_core::run` scope. Diagnostics are emitted before
/// returning `Err(Diagnosed)`.
pub fn parse(file: SourceFile, offset: usize, text: &str) -> Result<Query, Diagnosed> {
    let expr = parse::parse(file, offset, text)?;
    let needs = Needs::of(&expr);
    Ok(Query { expr, needs })
}
