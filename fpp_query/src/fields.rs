//! Discoverability: `--json` and `--fields`.
//!
//! `--json` prints the whole model; `--fields KIND` finds a real node of that
//! kind in the inputs and lists what it offers.

use fpp_ast::{MoveWalkable, Node, TransUnit, Visitor};
use serde_json::Value;
use std::ops::ControlFlow;

/// The whole syntax model as JSON: exactly the data a `$`-rooted path navigates.
pub fn json(units: &[TransUnit]) -> Result<String, String> {
    let value = Value::Array(
        units
            .iter()
            .map(|unit| serde_json::to_value(unit).expect("fpp_ast unit is not serializable"))
            .collect(),
    );
    serde_json::to_string_pretty(&value).map_err(|e| format!("cannot render JSON: {e}"))
}

/// With no kind, every selectable kind name. With a kind, the fields of the first
/// node of that kind found in the inputs.
pub fn describe(kind: &str, units: &[TransUnit]) -> String {
    if kind.is_empty() {
        let mut out = String::from("Selectable node kinds (use as a group's `node`):\n\n");
        for name in Node::KIND_NAMES {
            if crate::naming::kind_is_selectable(name) {
                out.push_str("  ");
                out.push_str(name);
                out.push('\n');
            }
        }
        out.push_str(
            "\n`--fields <KIND> <FILES>` lists one node's fields; `--json <FILES>` dumps the model.\n",
        );
        return out;
    }

    if !crate::naming::kind_is_selectable(kind) {
        return format!("`{kind}` is not a selectable node kind; run `--fields` for the list.\n");
    }

    let Some(found) = first_of_kind(kind, units) else {
        let where_to_look = if units.is_empty() {
            "name a file that contains one"
        } else {
            "the given files contain none; name one that does"
        };
        return format!(
            "`{kind}` fields are read from a real node, so {where_to_look}:\n\
             \x20   fpp-query --fields {kind} <FILES>\n\
             `--json <FILES>` dumps the whole model.\n"
        );
    };

    let mut out = format!("{kind}\n\n  fields (`$.<name>`):\n");
    match &found {
        Value::Object(map) => {
            let width = map.keys().map(String::len).max().unwrap_or(0);
            for (name, value) in map {
                out.push_str(&format!(
                    "    {name:<width$}  {}\n",
                    shape(value),
                    width = width
                ));
            }
        }
        other => out.push_str(&format!("    (this kind serializes as {})\n", shape(other))),
    }
    out.push_str(
        "\n  metadata (available on every kind):\n\
         \x20   $@          all annotation lines        $@post      `@<` lines\n\
         \x20   $@pre       `@` lines                   $@kind      this kind's name\n\
         \x20   $@file      source file URI             $@line      1-based start line\n\
         \x20   $@included  came from an `include`      $@scope     dotted enclosing scope\n\
         \x20   $@qualified scope and name              $@stem      default filename stem\n\
         \x20   $^          immediate parent            $^Kind      nearest enclosing Kind\n",
    );
    out
}

/// A one-word description of a serialized field's shape.
fn shape(value: &Value) -> String {
    match value {
        Value::Null => "absent here (an optional field)".to_string(),
        Value::Bool(x) => format!("boolean (here: {x})"),
        Value::Number(n) => format!("number (here: {n})"),
        Value::String(s) => format!("string (here: {s:?})"),
        Value::Array(items) => format!("list of {} here; use [i], len(), contains", items.len()),
        Value::Object(map) => match map.get("data") {
            // The string-leaf collapse: `Name`, `Ident`, and `LitString` compare
            // directly against a string.
            Some(Value::String(s)) => format!("string (here: {s:?})"),
            _ => format!(
                "record {{{}}}",
                map.keys().cloned().collect::<Vec<_>>().join(", ")
            ),
        },
    }
}

fn first_of_kind(kind: &str, units: &[TransUnit]) -> Option<Value> {
    struct Find<'k> {
        kind: &'k str,
    }
    impl<'ast> Visitor<'ast> for Find<'_> {
        type Break = Value;
        type State = ();

        fn super_visit(&self, a: &mut (), node: Node<'ast>) -> ControlFlow<Value> {
            if node.kind_name() == self.kind {
                return ControlFlow::Break(
                    serde_json::to_value(node).expect("fpp_ast node is not serializable"),
                );
            }
            node.walk(a, self)
        }
    }

    let find = Find { kind };
    match find.visit_trans_units(&mut (), units.iter()) {
        ControlFlow::Break(value) => Some(value),
        ControlFlow::Continue(()) => None,
    }
}
