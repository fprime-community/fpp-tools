//! Evaluator for the query language.
//!
//! Values are `serde_json::Value`s produced by `fpp_ast`'s `Serialize` impls.

use super::ast::{Expr, Meta, Path, RelOp, Root, Seg};
use crate::Diagnosed;
use fpp_core::{Diagnostic, Span};
use serde_json::Value;
use std::borrow::Cow;

/// The roots a query can reach: the matched node, its ancestors, and the
/// metadata that lives in the compiler context rather than in the node's fields.
pub trait Bindings {
    /// Kind name of the matched node, for error messages.
    fn kind(&self) -> &'static str;
    /// Span of the matched definition, attached to every diagnostic so the reader
    /// sees which definition tripped it.
    fn anchor(&self) -> Span;
    fn node(&self) -> &Value;
    /// `Value::Null` when there is no such ancestor.
    fn ancestor(&self, kind: Option<&str>) -> &Value;
    fn meta(&self, meta: Meta) -> Value;
}

struct Ev<'b, B: Bindings + ?Sized> {
    b: &'b B,
}

/// Evaluate `expr`. Diagnostics are emitted before returning `Err(Diagnosed)`.
pub fn eval<'b, B: Bindings + ?Sized>(expr: &Expr, b: &'b B) -> Result<Cow<'b, Value>, Diagnosed> {
    Ev { b }.expr(expr)
}

/// Evaluate `expr` as a predicate. Anything but a boolean is an error.
pub fn eval_bool<B: Bindings + ?Sized>(expr: &Expr, b: &B) -> Result<bool, Diagnosed> {
    let ev = Ev { b };
    let v = ev.expr(expr)?;
    match v.as_ref() {
        Value::Bool(x) => Ok(*x),
        other => {
            ev.err(
                expr.span(),
                format!("a query must be true or false, found {}", describe(other)),
            )
            .note("compare it against something, e.g. `$.name == \"Foo\"`")
            .emit();
            Err(Diagnosed)
        }
    }
}

/// Evaluate `expr` as a filename stem. Must be a non-empty string or an integer.
pub fn eval_stem<B: Bindings + ?Sized>(expr: &Expr, b: &B) -> Result<String, Diagnosed> {
    let ev = Ev { b };
    let v = ev.expr(expr)?;
    let stem = match v.as_ref() {
        Value::Number(n) => n.to_string(),
        other => ev
            .as_str(other, expr.span(), "a filename stem")?
            .to_string(),
    };
    if stem.is_empty() {
        ev.err(expr.span(), "a filename stem cannot be empty".to_string())
            .emit();
        return Err(Diagnosed);
    }
    // The same rule `--generate` suffixes are held to.
    if let Some(bad) = crate::naming::bad_filename_char(&stem) {
        ev.err(
            expr.span(),
            format!("this filename stem contains {bad:?}, which cannot appear in a filename"),
        )
        .note(format!("the stem evaluated to `{stem}`"))
        .note("generated files are written flat into the `-d` directory")
        .emit();
        return Err(Diagnosed);
    }
    Ok(stem)
}

impl<'b, B: Bindings + ?Sized> Ev<'b, B> {
    fn err(&self, span: Span, message: String) -> Diagnostic {
        span.error(message)
            .span_note(self.b.anchor(), "while matching this definition")
    }

    fn expr(&self, expr: &Expr) -> Result<Cow<'b, Value>, Diagnosed> {
        match expr {
            Expr::Str(s, _) => Ok(Cow::Owned(Value::String(s.clone()))),
            Expr::Int(n, _) => Ok(Cow::Owned(Value::from(*n))),
            Expr::Bool(x, _) => Ok(Cow::Owned(Value::Bool(*x))),
            Expr::Null(_) => Ok(Cow::Owned(Value::Null)),
            Expr::Path(p) => self.path(p),
            Expr::Not(inner, span) => {
                let v = self.expr(inner)?;
                Ok(Cow::Owned(Value::Bool(!self.as_bool(&v, *span)?)))
            }
            Expr::And(a, b, span) => {
                let lhs = self.expr(a)?;
                if !self.as_bool(&lhs, *span)? {
                    return Ok(Cow::Owned(Value::Bool(false)));
                }
                let rhs = self.expr(b)?;
                Ok(Cow::Owned(Value::Bool(self.as_bool(&rhs, *span)?)))
            }
            Expr::Or(a, b, span) => {
                let lhs = self.expr(a)?;
                if self.as_bool(&lhs, *span)? {
                    return Ok(Cow::Owned(Value::Bool(true)));
                }
                let rhs = self.expr(b)?;
                Ok(Cow::Owned(Value::Bool(self.as_bool(&rhs, *span)?)))
            }
            Expr::Rel { op, lhs, rhs, span } => {
                let l = self.expr(lhs)?;
                let r = self.expr(rhs)?;
                Ok(Cow::Owned(Value::Bool(self.rel(*op, &l, &r, *span)?)))
            }
            Expr::List(items, _) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(self.expr(item)?.into_owned());
                }
                Ok(Cow::Owned(Value::Array(out)))
            }
            Expr::In { lhs, rhs, span } => {
                let l = self.expr(lhs)?;
                let r = self.expr(rhs)?;
                let Value::Array(items) = r.as_ref() else {
                    self.err(
                        rhs.span(),
                        format!("`in` needs a list on the right, found {}", describe(&r)),
                    )
                    .note("write a set (`[\"Active\", \"Queued\"]`) or a list-valued path (`$@`)")
                    .emit();
                    return Err(Diagnosed);
                };
                for item in items {
                    if self.equal(&l, item, *span)? {
                        return Ok(Cow::Owned(Value::Bool(true)));
                    }
                }
                Ok(Cow::Owned(Value::Bool(false)))
            }
            Expr::Cat(a, b, span) => {
                let l = self.expr(a)?;
                let r = self.expr(b)?;
                if let (Some(x), Some(y)) = (l.as_i64(), r.as_i64()) {
                    return Ok(Cow::Owned(Value::from(x.saturating_add(y))));
                }
                let mut s = self.stringify(&l, *span)?;
                s.push_str(&self.stringify(&r, *span)?);
                Ok(Cow::Owned(Value::String(s)))
            }
            Expr::Call { name, args, span } => self.call(name, args, *span),
        }
    }

    fn path(&self, p: &Path) -> Result<Cow<'b, Value>, Diagnosed> {
        match &p.root {
            Root::Node => {
                let root = self.b.node();
                self.navigate(root, &p.segs, self.b.kind())
                    .map(Cow::Borrowed)
            }
            Root::Ancestor(kind) => {
                let root = self.b.ancestor(kind.as_deref());
                if root.is_null() && !p.segs.is_empty() {
                    self.err(
                        p.span,
                        match kind {
                            Some(k) => format!("no enclosing `{k}`"),
                            None => "no enclosing definition".to_string(),
                        },
                    )
                    .note(format!(
                        "guard it: `${}{} != null && ...`",
                        "^",
                        kind.as_deref().unwrap_or("")
                    ))
                    .emit();
                    return Err(Diagnosed);
                }
                let desc = kind.as_deref().unwrap_or("the enclosing node");
                self.navigate(root, &p.segs, desc).map(Cow::Borrowed)
            }
            Root::Meta(meta) => {
                let value = self.b.meta(*meta);
                if p.segs.is_empty() {
                    return Ok(Cow::Owned(value));
                }
                // Metadata is scalar or a list of strings; it has no fields.
                self.navigate(&value, &p.segs, "metadata")
                    .map(|v| Cow::Owned(v.clone()))
            }
        }
    }

    fn navigate<'v>(
        &self,
        root: &'v Value,
        segs: &[Seg],
        root_desc: &str,
    ) -> Result<&'v Value, Diagnosed> {
        let mut cur = root;
        let mut owner = root_desc.to_string();
        for seg in segs {
            match seg {
                Seg::Field(name, span) => {
                    let Value::Object(map) = cur else {
                        self.err(
                            *span,
                            format!("cannot read field `{name}` of {}", describe(cur)),
                        )
                        .note(match cur {
                            Value::Null => {
                                "the value is absent; guard it with `!= null` first".to_string()
                            }
                            Value::Array(_) => {
                                "index it (`[0]`), or use `contains` / `len`".to_string()
                            }
                            _ => format!("`{owner}` is not a record"),
                        })
                        .emit();
                        return Err(Diagnosed);
                    };
                    let Some(next) = map.get(name) else {
                        self.err(*span, format!("`{owner}` has no field `{name}`"))
                            .note(format!(
                                "fields of `{owner}`: {}",
                                map.keys().cloned().collect::<Vec<_>>().join(", ")
                            ))
                            .emit();
                        return Err(Diagnosed);
                    };
                    owner = name.clone();
                    cur = next;
                }
                Seg::Index(index, span) => {
                    let Value::Array(items) = cur else {
                        self.err(
                            *span,
                            format!("cannot index {}, which is not a list", describe(cur)),
                        )
                        .emit();
                        return Err(Diagnosed);
                    };
                    let Some(next) = items.get(*index) else {
                        self.err(
                            *span,
                            format!(
                                "index {index} is out of range: `{owner}` has {} element{}",
                                items.len(),
                                if items.len() == 1 { "" } else { "s" }
                            ),
                        )
                        .emit();
                        return Err(Diagnosed);
                    };
                    cur = next;
                }
            }
        }
        Ok(cur)
    }

    fn call(&self, name: &str, args: &[Expr], span: Span) -> Result<Cow<'b, Value>, Diagnosed> {
        match name {
            "len" => {
                let v = self.expr(&args[0])?;
                let n = match v.as_ref() {
                    Value::Array(items) => items.len(),
                    other => self.as_str(other, args[0].span(), "len")?.chars().count(),
                };
                Ok(Cow::Owned(Value::from(
                    u64::try_from(n).unwrap_or(u64::MAX),
                )))
            }
            "lower" | "upper" => {
                let v = self.expr(&args[0])?;
                let s = self.as_str(&v, args[0].span(), name)?;
                let out = if name == "lower" {
                    s.to_ascii_lowercase()
                } else {
                    s.to_ascii_uppercase()
                };
                Ok(Cow::Owned(Value::String(out)))
            }
            "replace" => {
                let subject = self.expr(&args[0])?;
                let from = self.expr(&args[1])?;
                let to = self.expr(&args[2])?;
                let subject = self.as_str(&subject, args[0].span(), "replace")?;
                let from = self.as_str(&from, args[1].span(), "replace")?;
                if from.is_empty() {
                    self.err(
                        args[1].span(),
                        "`replace` needs something to look for".to_string(),
                    )
                    .emit();
                    return Err(Diagnosed);
                }
                let to = self.as_str(&to, args[2].span(), "replace")?;
                Ok(Cow::Owned(Value::String(subject.replace(from, to))))
            }
            "join" => {
                let list = self.expr(&args[0])?;
                let sep = self.expr(&args[1])?;
                let sep = self.as_str(&sep, args[1].span(), "join")?.to_string();
                let Value::Array(items) = list.as_ref() else {
                    self.err(
                        args[0].span(),
                        format!("`join` needs a list, found {}", describe(&list)),
                    )
                    .emit();
                    return Err(Diagnosed);
                };
                let mut parts = Vec::with_capacity(items.len());
                for item in items {
                    parts.push(self.as_str(item, args[0].span(), "join")?.to_string());
                }
                Ok(Cow::Owned(Value::String(parts.join(&sep))))
            }
            // The parser rejects unknown names, so this is unreachable.
            _ => {
                self.err(span, format!("unknown function `{name}`")).emit();
                Err(Diagnosed)
            }
        }
    }

    fn rel(&self, op: RelOp, lhs: &Value, rhs: &Value, span: Span) -> Result<bool, Diagnosed> {
        // `$@ contains "x"` and `$.implements contains "Svc.Sched"` read over the
        // elements, so the string operators hold when they hold for any element.
        if op.lifts_over_lists()
            && let Value::Array(items) = lhs
        {
            for item in items {
                if self.rel(op, item, rhs, span)? {
                    return Ok(true);
                }
            }
            return Ok(false);
        }

        match op {
            RelOp::Eq | RelOp::Ne => {
                let equal = self.equal(lhs, rhs, span)?;
                Ok(if op == RelOp::Eq { equal } else { !equal })
            }
            RelOp::Lt | RelOp::Le | RelOp::Gt | RelOp::Ge => {
                // Every number the grammar or this evaluator produces is an
                // integer (`$@line`, `len()`), so there is no float ordering to
                // worry about; anything else compares as text.
                let ordering = if let (Some(a), Some(b)) = (lhs.as_i64(), rhs.as_i64()) {
                    a.cmp(&b)
                } else {
                    let a = self.as_str(lhs, span, op.spelling())?;
                    let b = self.as_str(rhs, span, op.spelling())?;
                    a.cmp(b)
                };
                Ok(match op {
                    RelOp::Lt => ordering.is_lt(),
                    RelOp::Le => ordering.is_le(),
                    RelOp::Gt => ordering.is_gt(),
                    _ => ordering.is_ge(),
                })
            }
            RelOp::Contains | RelOp::StartsWith | RelOp::EndsWith | RelOp::Matches => {
                let a = self.as_str(lhs, span, op.spelling())?;
                let b = self.as_str(rhs, span, op.spelling())?;
                Ok(match op {
                    RelOp::Contains => a.contains(b),
                    RelOp::StartsWith => a.starts_with(b),
                    RelOp::EndsWith => a.ends_with(b),
                    _ => glob_match(b, a),
                })
            }
        }
    }

    fn equal(&self, lhs: &Value, rhs: &Value, span: Span) -> Result<bool, Diagnosed> {
        // Comparing against `null` is how an absent `Option` is tested, so it is
        // the one cross-type comparison that is allowed.
        if lhs.is_null() || rhs.is_null() {
            return Ok(lhs.is_null() && rhs.is_null());
        }
        if let (Value::Bool(a), Value::Bool(b)) = (lhs, rhs) {
            return Ok(a == b);
        }
        if let (Some(a), Some(b)) = (lhs.as_i64(), rhs.as_i64()) {
            return Ok(a == b);
        }
        if lhs.is_boolean() != rhs.is_boolean() || lhs.is_number() != rhs.is_number() {
            self.err(
                span,
                format!("cannot compare {} with {}", describe(lhs), describe(rhs)),
            )
            .emit();
            return Err(Diagnosed);
        }
        let a = self.as_str(lhs, span, "==")?;
        let b = self.as_str(rhs, span, "==")?;
        Ok(a == b)
    }

    fn as_bool(&self, v: &Value, span: Span) -> Result<bool, Diagnosed> {
        match v {
            Value::Bool(x) => Ok(*x),
            other => {
                self.err(
                    span,
                    format!("expected true or false, found {}", describe(other)),
                )
                .emit();
                Err(Diagnosed)
            }
        }
    }

    /// Coerce to a string. An object carrying a single `data` string collapses to
    /// it, so `$.name == "Foo"` works directly on `Name`, `Ident`, and
    /// `LitString` nodes.
    fn as_str<'v>(&self, v: &'v Value, span: Span, what: &str) -> Result<&'v str, Diagnosed> {
        match v {
            Value::String(s) => Ok(s),
            Value::Object(map) => match map.get("data") {
                Some(Value::String(s)) => Ok(s),
                _ => {
                    self.err(span, format!("`{what}` needs a string, found a record"))
                        .note(format!(
                            "fields available: {}",
                            map.keys().cloned().collect::<Vec<_>>().join(", ")
                        ))
                        .emit();
                    Err(Diagnosed)
                }
            },
            other => {
                self.err(
                    span,
                    format!("`{what}` needs a string, found {}", describe(other)),
                )
                .emit();
                Err(Diagnosed)
            }
        }
    }

    fn stringify(&self, v: &Value, span: Span) -> Result<String, Diagnosed> {
        match v {
            Value::Number(n) => Ok(n.to_string()),
            Value::Bool(x) => Ok(x.to_string()),
            other => Ok(self.as_str(other, span, "+")?.to_string()),
        }
    }
}

fn describe(v: &Value) -> String {
    match v {
        Value::Null => "nothing".to_string(),
        Value::Bool(x) => format!("`{x}`"),
        Value::Number(n) => format!("the number {n}"),
        Value::String(s) => format!("the string `{s}`"),
        Value::Array(items) => format!("a list of {} element(s)", items.len()),
        Value::Object(map) => match map.get("data") {
            Some(Value::String(s)) => format!("the string `{s}`"),
            _ => "a record".to_string(),
        },
    }
}

/// Glob match for the `matches` operator: `*` is any run of characters, `?` is
/// exactly one, and `\` escapes either.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    // Backtracking over `*` only; patterns here are short and literal-heavy.
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        match p.get(pi) {
            Some('*') => {
                star = Some((pi, ti));
                pi += 1;
            }
            Some('?') => {
                pi += 1;
                ti += 1;
            }
            Some('\\') if pi + 1 < p.len() => {
                if p[pi + 1] == t[ti] {
                    pi += 2;
                    ti += 1;
                } else if let Some((sp, st)) = star {
                    pi = sp + 1;
                    ti = st + 1;
                    star = Some((sp, st + 1));
                } else {
                    return false;
                }
            }
            Some(c) if *c == t[ti] => {
                pi += 1;
                ti += 1;
            }
            _ => {
                if let Some((sp, st)) = star {
                    pi = sp + 1;
                    ti = st + 1;
                    star = Some((sp, st + 1));
                } else {
                    return false;
                }
            }
        }
    }
    while p.get(pi) == Some(&'*') {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::glob_match;

    #[test]
    fn glob() {
        assert!(glob_match("*Ac.hpp", "FooArrayAc.hpp"));
        assert!(glob_match("Foo?", "Foos"));
        assert!(!glob_match("Foo?", "Foo"));
        assert!(glob_match("*", ""));
        assert!(glob_match("a*b*c", "axxbyyc"));
        assert!(!glob_match("a*b*c", "axxbyy"));
        assert!(glob_match(r"a\*b", "a*b"));
        assert!(!glob_match(r"a\*b", "axb"));
        assert!(glob_match("static-*", "static-tlm-packetizer"));
    }
}
