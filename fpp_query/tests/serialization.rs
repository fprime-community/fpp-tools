//! The `fpp_ast` serialization contract that this crate's query language rests on.

use fpp_core::CompilerContext;
use fpp_errors::WriteEmitter;
use serde_json::Value;

/// Parse `source` and return its serialized syntax tree.
fn json(source: &str) -> Value {
    let mut sink: Vec<u8> = Vec::new();
    let mut ctx = CompilerContext::new(WriteEmitter::new(&mut sink));
    let value = fpp_core::run(&mut ctx, || {
        let file = fpp_core::SourceFile::new("<test>", source.to_string());
        let unit = fpp_parser::parse(file, |p| p.trans_unit(), None);
        serde_json::to_value(&unit).expect("grammar must serialize")
    });
    assert!(
        sink.is_empty(),
        "test source did not parse cleanly:\n{}",
        String::from_utf8_lossy(&sink)
    );
    value
}

/// The first object under `key`, at any depth.
fn find<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => {
            if let Some(found) = map.get(key) {
                return Some(found);
            }
            map.values().find_map(|v| find(v, key))
        }
        Value::Array(items) => items.iter().find_map(|v| find(v, key)),
        _ => None,
    }
}

fn contains_key(value: &Value, key: &str) -> bool {
    find(value, key).is_some()
}

/// `key` of the first node tagged `kind`. Needed because the enclosing
/// `module` has fields of the same name (`name`, `members`) and a plain
/// depth-first search would find those first.
fn field_of<'a>(value: &'a Value, kind: &str, key: &str) -> Option<&'a Value> {
    find(value, kind).and_then(|node| node.get(key))
}

#[test]
fn context_handles_are_not_serialized() {
    let value = json("module M { deployment topology T { } }");
    // A node handle and a span index only mean something inside the compiler
    // context that produced them, so neither belongs in the data.
    assert!(!contains_key(&value, "node_id"));
    assert!(!contains_key(&value, "inner_span"));
}

#[test]
fn union_variants_are_tagged_with_the_node_kind() {
    let value = json("module M { deployment topology T { } }");
    // The tag is what lets a consumer see which kind a member is.
    assert!(contains_key(&value, "DefModule"));
    assert!(contains_key(&value, "DefTopology"));
}

#[test]
fn ignored_fields_are_still_serialized() {
    // `is_deployment` carries `#[visitable(ignore)]`: the walk skips it, but it
    // is still a real field.
    let value = json("module M { deployment topology T { } }");
    assert_eq!(find(&value, "is_deployment"), Some(&Value::Bool(true)));
}

#[test]
fn unit_enum_variants_serialize_as_their_name() {
    let value = json("module M { active component C { } }");
    assert_eq!(
        find(&value, "kind"),
        Some(&Value::String("Active".to_string()))
    );
}

#[test]
fn absent_options_serialize_as_null() {
    // An external state machine has no body; the `Option` is how that is told
    // apart from an empty one.
    let external = json("module M { state machine SM }");
    assert_eq!(
        field_of(&external, "DefStateMachine", "members"),
        Some(&Value::Null)
    );

    let bodied = json("module M { state machine SM { initial enter S \n state S } }");
    assert!(matches!(
        field_of(&bodied, "DefStateMachine", "members"),
        Some(Value::Array(_))
    ));
}

#[test]
fn qualified_identifiers_serialize_as_a_dotted_string() {
    let value = json("module M { instance i: A.B.C base id 0 }");
    assert_eq!(
        find(&value, "component"),
        Some(&Value::String("A.B.C".to_string()))
    );
}

#[test]
fn string_leaves_keep_their_data_field() {
    // `Name`, `Ident`, and `LitString` serialize as `{"data": "..."}`; consumers
    // rely on that shape to collapse them to a plain string.
    let value = json("module M { deployment topology T { } }");
    let name = field_of(&value, "DefTopology", "name").expect("a name");
    assert_eq!(name.get("data"), Some(&Value::String("T".to_string())));
}

#[test]
fn annotations_are_serialized_alongside_the_fields_they_annotate() {
    // Annotations live in the compiler context keyed by node handle, not as a
    // struct field, so a plain `#[derive(Serialize)]` cannot see them on its own --
    // the `#[ast]` macro's injected field is what carries them into the JSON.
    let value = json(
        "module M {\n\
         @ static-tlm-packetizer\n\
         @ opcodes are assigned statically\n\
         passive component C { }\n\
         @< a trailing note\n\
         array A = [1] U8\n\
         }",
    );
    let component = field_of(&value, "DefComponent", "annotations").expect("annotations");
    assert_eq!(
        component.get("pre"),
        Some(&Value::Array(vec![
            Value::String("static-tlm-packetizer".to_string()),
            Value::String("opcodes are assigned statically".to_string()),
        ]))
    );
    // `@<` is a POST annotation on the definition it trails -- here, `component C`,
    // not the `array A` that follows it.
    assert_eq!(
        component.get("post"),
        Some(&Value::Array(vec![Value::String(
            "a trailing note".to_string()
        )]))
    );

    // A node that carries neither still has the field, with both sides empty -- the
    // same convention an absent `Option` follows (`null`, never an omitted key).
    let array = field_of(&value, "DefArray", "annotations").expect("annotations");
    assert_eq!(array.get("pre"), Some(&Value::Array(vec![])));
    assert_eq!(array.get("post"), Some(&Value::Array(vec![])));
}

#[test]
fn annotations_are_reachable_on_nested_nodes_unlike_the_query_languages_meta_root() {
    // The query language's `$@pre`/`$@post` roots only ever read the top-level
    // matched node. Because annotations are serialized in place, a nested node's
    // annotations are reachable too -- `$.members[0].DefComponent.annotations.pre`,
    // which `$@` alone cannot express.
    let value = json("module M {\n  @ tag\n  passive component C { }\n}");
    let module_members = field_of(&value, "DefModule", "members").expect("members");
    let Value::Array(members) = module_members else {
        panic!("members is not an array")
    };
    let component = members[0].get("DefComponent").expect("a DefComponent");
    assert_eq!(
        component.get("annotations").and_then(|a| a.get("pre")),
        Some(&Value::Array(vec![Value::String("tag".to_string())]))
    );
}
