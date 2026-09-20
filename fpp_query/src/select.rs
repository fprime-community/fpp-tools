//! The single deep walk that matches nodes by kind and evaluates each group.

use crate::Diagnosed;
use crate::query::eval::{self, Bindings};
use crate::query::{Meta, Query};
use fpp_ast::{AstNode, MoveWalkable, Node, TransUnit, Visitor};
use fpp_core::{Annotated, Span, Spanned};
use serde_json::Value;
use std::ops::ControlFlow;

/// The kinds that name a scope, and therefore contribute to `$@scope`
const SCOPE_KINDS: &[&str] = &["DefModule", "DefComponent", "DefEnum", "DefStateMachine"];

/// The kinds that contribute a `Parent_` prefix to a generated filename. Module
/// nesting does not: `module M { component C { array A } }` yields
/// `C_AArrayAc.hpp`, matching what fpp-to-cpp writes.
const STEM_PREFIX_KINDS: &[&str] = &["DefComponent", "DefStateMachine"];

/// One group: a kind to match, optional `where`/`name` expressions, and the
/// suffixes to append.
pub struct Group {
    pub kind: String,
    pub where_: Option<Query>,
    pub name: Option<Query>,
    pub suffixes: Vec<String>,
}

impl Group {
    /// Roots this group's expressions need materialized, unioned across `where`
    /// and `name` so one pass covers both.
    fn needs(&self) -> crate::query::Needs {
        crate::query::Needs::union(
            [self.where_.as_ref(), self.name.as_ref()]
                .into_iter()
                .flatten()
                .map(|q| q.needs.clone()),
        )
    }
}

/// Filename stems matched per group, parallel to the group list.
pub struct Matches {
    pub stems: Vec<Vec<String>>,
}

/// Run every group over every translation unit.
///
/// Returns `Err(Diagnosed)` after emitting a diagnostic if a query fails to
/// evaluate.
pub fn run<'ast>(
    groups: &[Group],
    units: impl Iterator<Item = &'ast TransUnit>,
) -> Result<Matches, Diagnosed> {
    let needs: Vec<_> = groups.iter().map(Group::needs).collect();
    let selector = Selector { groups, needs };
    let mut walk = Walk {
        stack: Vec::new(),
        stems: vec![Vec::new(); groups.len()],
        failed: false,
    };
    let _ = selector.visit_trans_units(&mut walk, units);
    if walk.failed {
        return Err(Diagnosed);
    }
    Ok(Matches { stems: walk.stems })
}

struct Selector<'g> {
    groups: &'g [Group],
    needs: Vec<crate::query::Needs>,
}

struct Walk<'ast> {
    /// Enclosing nodes, outermost first, excluding the node being visited.
    stack: Vec<Node<'ast>>,
    stems: Vec<Vec<String>>,
    failed: bool,
}

impl<'ast> Visitor<'ast> for Selector<'_> {
    type Break = ();
    type State = Walk<'ast>;

    /// Overriding `super_visit` turns the walk deep: every node reaches here.
    fn super_visit(&self, a: &mut Walk<'ast>, node: Node<'ast>) -> ControlFlow<()> {
        let kind = node.kind_name();
        for (index, group) in self.groups.iter().enumerate() {
            if group.kind != kind {
                continue;
            }
            match self.select(group, &self.needs[index], node, &a.stack) {
                Ok(Some(stem)) => a.stems[index].push(stem),
                Ok(None) => {}
                Err(Diagnosed) => {
                    a.failed = true;
                    return ControlFlow::Break(());
                }
            }
        }

        a.stack.push(node);
        let flow = node.walk(a, self);
        a.stack.pop();
        flow
    }
}

impl Selector<'_> {
    /// `Ok(None)` when the predicate rejected the node.
    fn select<'ast>(
        &self,
        group: &Group,
        needs: &crate::query::Needs,
        node: Node<'ast>,
        stack: &[Node<'ast>],
    ) -> Result<Option<String>, Diagnosed> {
        let stem = default_stem(node, stack);

        if group.where_.is_none() && group.name.is_none() {
            return required_stem(stem, node).map(Some);
        }

        let node_json = needs.node.then(|| to_json(node));
        let ancestors: Vec<(Option<String>, Value)> = needs
            .ancestors
            .iter()
            .map(|kind| {
                let found = find_ancestor(stack, kind.as_deref());
                (kind.clone(), found.map_or(Value::Null, to_json))
            })
            .collect();

        let bindings = NodeBindings {
            node,
            stack,
            kind: node.kind_name(),
            stem: stem.as_deref(),
            node_json: node_json.as_ref(),
            ancestors: &ancestors,
        };

        if let Some(query) = &group.where_
            && !eval::eval_bool(&query.expr, &bindings)?
        {
            return Ok(None);
        }

        match &group.name {
            Some(query) => eval::eval_stem(&query.expr, &bindings).map(Some),
            None => required_stem(stem, node).map(Some),
        }
    }
}

/// The default filename stem: the definition's own name, prefixed recursively by
/// the names of enclosing components and state machines.
///
/// `None` for a node that introduces no name of its own (`SpecInclude`, `Expr`,
/// `Connection`, ...). Such a kind is still selectable, but only with an explicit
/// `name` expression.
fn default_stem(node: Node<'_>, stack: &[Node<'_>]) -> Option<String> {
    let own = node.name()?;
    let mut parts: Vec<&str> = stack
        .iter()
        .filter(|a| STEM_PREFIX_KINDS.contains(&a.kind_name()))
        .filter_map(Node::name)
        .collect();
    parts.push(own);
    Some(parts.join("_"))
}

/// Turn an absent default stem into an error rather than an empty filename: a
/// nameless kind would emit `<dir>/<suffix>`, a path every match of the group
/// collapses onto.
fn required_stem(stem: Option<String>, node: Node<'_>) -> Result<String, Diagnosed> {
    match stem {
        Some(stem) => Ok(stem),
        None => {
            node.span()
                .error(format!(
                    "`{}` introduces no name, so there is nothing to build a filename from",
                    node.kind_name()
                ))
                .note("give the group a `name` expression")
                .emit();
            Err(Diagnosed)
        }
    }
}

fn scope_of(stack: &[Node<'_>]) -> String {
    stack
        .iter()
        .filter(|a| SCOPE_KINDS.contains(&a.kind_name()))
        .filter_map(Node::name)
        .collect::<Vec<_>>()
        .join(".")
}

/// Nearest enclosing node of `kind`, or the immediate parent when `kind` is
/// `None`.
fn find_ancestor<'ast>(stack: &[Node<'ast>], kind: Option<&str>) -> Option<Node<'ast>> {
    match kind {
        None => stack.last().copied(),
        Some(kind) => stack.iter().rev().find(|a| a.kind_name() == kind).copied(),
    }
}

/// `fpp_ast` serialization is infallible: the grammar has no non-string map keys
/// and no floating-point values, the two things `to_value` rejects.
fn to_json(node: Node<'_>) -> Value {
    serde_json::to_value(node).expect("fpp_ast node is not serializable as JSON")
}

struct NodeBindings<'a, 'ast> {
    node: Node<'ast>,
    stack: &'a [Node<'ast>],
    kind: &'static str,
    stem: Option<&'a str>,
    node_json: Option<&'a Value>,
    ancestors: &'a [(Option<String>, Value)],
}

impl Bindings for NodeBindings<'_, '_> {
    fn kind(&self) -> &'static str {
        self.kind
    }

    fn anchor(&self) -> Span {
        self.node.span()
    }

    fn node(&self) -> &Value {
        // `Needs` decided this was reachable, so the walk materialized it.
        self.node_json
            .expect("a `$`-rooted path was evaluated without the node materialized")
    }

    fn ancestor(&self, kind: Option<&str>) -> &Value {
        self.ancestors
            .iter()
            .find(|(k, _)| k.as_deref() == kind)
            .map(|(_, v)| v)
            .expect("a `$^`-rooted path was evaluated without the ancestor materialized")
    }

    fn meta(&self, meta: Meta) -> Value {
        let id = self.node.id();
        let strings = |v: Vec<String>| Value::Array(v.into_iter().map(Value::String).collect());
        match meta {
            Meta::All => {
                let mut lines = id.pre_annotation();
                lines.extend(id.post_annotation());
                strings(lines)
            }
            Meta::Pre => strings(id.pre_annotation()),
            Meta::Post => strings(id.post_annotation()),
            Meta::Kind => Value::String(self.kind.to_string()),
            Meta::File => Value::String(self.node.span().file().uri()),
            Meta::Line => Value::from(self.node.span().start().line() + 1),
            // A node parsed out of an `include`d file carries the span of the
            // specifier that pulled it in.
            Meta::Included => Value::Bool(self.node.span().including_span().is_some()),
            Meta::Scope => Value::String(scope_of(self.stack)),
            Meta::Qualified => {
                let scope = scope_of(self.stack);
                match (scope.is_empty(), self.node.name()) {
                    (_, None) => Value::Null,
                    (true, Some(name)) => Value::String(name.to_string()),
                    (false, Some(name)) => Value::String(format!("{scope}.{name}")),
                }
            }
            Meta::Stem => match self.stem {
                Some(stem) => Value::String(stem.to_string()),
                None => Value::Null,
            },
        }
    }
}
