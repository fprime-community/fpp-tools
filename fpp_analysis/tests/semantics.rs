//! Tests for the data recorded in the semantic data structures.
//!
//! The `integration` test suite compares diagnostics; these tests instead
//! assert the values that the semantic structures carry, so that fields whose
//! only consumer is downstream tooling (types, formats, limits, throttles,
//! default values, instance attributes) stay correct.

use fpp_analysis::semantics::{
    Command, ComponentInstance, NonParamKind, ParamKind, Symbol, SymbolInterface, Type, Value,
};
use fpp_analysis::{Analysis, add_state_enums, check_semantics};
use fpp_ast::{QueueFull, TlmChannelLimitKind, TlmChannelUpdate};
use fpp_core::{SourceFile, Spanned};
use std::sync::Arc;

/// Analyze a source string and hand the analysis to `f`. Panics if the input
/// produced any diagnostic.
fn with_analysis(src: &str, f: impl FnOnce(&Analysis)) {
    let mut diagnostics = vec![];
    let mut ctx = fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
    fpp_core::run(&mut ctx, || {
        let source = SourceFile::new("semantics_test.fpp", src.to_string());
        let mut ast = fpp_parser::parse(source, |p| p.trans_unit(), None);
        let mut a = Analysis::new();
        add_state_enums(&mut ast);
        let _ = check_semantics(&mut a, vec![&ast]);
        f(&a);
    });
    let output = String::from_utf8(diagnostics).expect("diagnostics are UTF-8");
    assert_eq!(output, "", "expected no diagnostics");
}

/// The single component of the analysis.
fn the_component(a: &Analysis) -> &fpp_analysis::semantics::Component {
    a.component_map
        .values()
        .next()
        .expect("one component was analyzed")
}

/// The single component instance of the analysis.
fn the_instance(a: &Analysis) -> &ComponentInstance {
    a.component_instance_map
        .values()
        .next()
        .expect("one component instance was analyzed")
}

const PORTS: &str = r#"
module Fw {
  port Cmd
  port CmdReg
  port CmdResponse
  port Log
  port LogText
  port PrmGet
  port PrmSet
  port Time
  port Tlm
  port DpRequest
  port DpResponse
  port DpSend
}
"#;

#[test]
fn tlm_channel_carries_type_update_format_and_limits() {
    let src = format!(
        r#"{PORTS}
passive component C {{
  telemetry port tlmOut
  time get port timeGetOut

  telemetry T: U32 \
    id 0x10 \
    update on change \
    format "v = {{}}" \
    low {{ red 1, yellow 2 }} \
    high {{ red 10 }}
}}
"#
    );
    with_analysis(&src, |a| {
        let component = the_component(a);
        let channel = component
            .tlm_channel_map
            .get(&0x10)
            .expect("channel at id 0x10");
        assert_eq!(channel.get_name(), "T");
        assert!(matches!(
            channel.channel_type.as_ref(),
            Type::PrimitiveInt(fpp_ast::IntegerKind::U32)
        ));
        assert!(matches!(channel.update, TlmChannelUpdate::OnChange));
        assert_eq!(channel.format.as_ref().expect("a format").len(), 1);

        assert_eq!(channel.low_limits.len(), 2);
        let (_, red_low) = channel
            .low_limits
            .get(&TlmChannelLimitKind::Red)
            .expect("a red low limit");
        assert!(matches!(red_low, Value::Integer(v) if v.0 == 1));
        assert!(
            channel
                .low_limits
                .contains_key(&TlmChannelLimitKind::Yellow)
        );
        assert_eq!(channel.high_limits.len(), 1);
        let (_, red_high) = channel
            .high_limits
            .get(&TlmChannelLimitKind::Red)
            .expect("a red high limit");
        assert!(matches!(red_high, Value::Integer(v) if v.0 == 10));

        // A channel with no update specifier defaults to Always.
        assert!(component.tlm_channel_name_map.contains_key("T"));
    });
}

#[test]
fn tlm_channel_update_defaults_to_always() {
    let src = format!(
        r#"{PORTS}
passive component C {{
  telemetry port tlmOut
  time get port timeGetOut
  telemetry T: U32
}}
"#
    );
    with_analysis(&src, |a| {
        let channel = the_component(a)
            .tlm_channel_map
            .get(&0)
            .expect("channel at the default id");
        assert!(matches!(channel.update, TlmChannelUpdate::Always));
        assert!(channel.format.is_none());
        assert!(channel.low_limits.is_empty());
        assert!(channel.high_limits.is_empty());
    });
}

#[test]
fn event_carries_format_and_throttle() {
    let src = format!(
        r#"{PORTS}
passive component C {{
  event port eventOut
  text event port textEventOut
  time get port timeGetOut

  event E0(arg: U32) severity activity low id 0x10 format "saw {{}}"

  event E1 severity warning high id 0x11 format "throttled" \
    throttle 4 every {{ seconds = 2, useconds = 3 }}
}}
"#
    );
    with_analysis(&src, |a| {
        let component = the_component(a);
        let e0 = component.event_map.get(&0x10).expect("event at 0x10");
        assert_eq!(e0.get_name(), "E0");
        assert_eq!(e0.format.len(), 1);
        assert!(e0.throttle.is_none());

        let e1 = component.event_map.get(&0x11).expect("event at 0x11");
        let throttle = e1.throttle.as_ref().expect("a throttle");
        assert_eq!(throttle.count, 4);
        let every = throttle.every.as_ref().expect("an interval");
        assert_eq!(every.seconds, 2);
        assert_eq!(every.useconds, 3);
    });
}

#[test]
fn param_carries_type_default_and_implies_commands() {
    let src = format!(
        r#"{PORTS}
passive component C {{
  command recv port cmdIn
  command reg port cmdRegOut
  command resp port cmdResponseOut
  param get port prmGetOut
  param set port prmSetOut

  param P: U32 default 7 id 0x10 set opcode 0x20 save opcode 0x21
  external param Q: U16 id 0x11 set opcode 0x22 save opcode 0x23
}}
"#
    );
    with_analysis(&src, |a| {
        let component = the_component(a);
        let p = component.param_map.get(&0x10).expect("param at 0x10");
        assert_eq!(p.get_name(), "P");
        assert!(matches!(
            p.param_type.as_ref(),
            Type::PrimitiveInt(fpp_ast::IntegerKind::U32)
        ));
        assert!(
            matches!(p.default.as_ref(), Some(Value::PrimitiveInteger(v)) if v.value == 7),
            "default value is the converted parameter value, got {:?}",
            p.default
        );
        assert_eq!(p.set_opcode, 0x20);
        assert_eq!(p.save_opcode, 0x21);
        assert!(!p.is_external);
        assert!(component.has_external_parameters());

        // Each parameter implies a set command and a save command, named after
        // the parameter and backed by its specifier node.
        let set = component.command_map.get(&0x20).expect("set command");
        let save = component.command_map.get(&0x21).expect("save command");
        assert_eq!(set.get_name(), "P_PRM_SET");
        assert_eq!(save.get_name(), "P_PRM_SAVE");
        assert!(matches!(
            set,
            Command::Param {
                kind: ParamKind::Set,
                ..
            }
        ));
        assert!(matches!(
            save,
            Command::Param {
                kind: ParamKind::Save,
                ..
            }
        ));
        assert_eq!(set.get_loc(), p.get_loc());
        assert!(!set.is_async());
    });
}

#[test]
fn command_carries_kind() {
    let src = format!(
        r#"{PORTS}
active component C {{
  command recv port cmdIn
  command reg port cmdRegOut
  command resp port cmdResponseOut

  async command A opcode 0x10 priority 3 assert
  guarded command G opcode 0x11
  sync command S opcode 0x12
}}
"#
    );
    with_analysis(&src, |a| {
        let component = the_component(a);
        let async_command = component.command_map.get(&0x10).expect("command at 0x10");
        assert_eq!(async_command.get_name(), "A");
        assert!(async_command.is_async());
        match async_command {
            Command::NonParam {
                kind:
                    NonParamKind::Async {
                        priority,
                        queue_full,
                    },
                ..
            } => {
                assert_eq!(*priority, Some(3));
                assert!(matches!(queue_full, QueueFull::Assert));
            }
            other => panic!("expected an async non-param command, got {other:?}"),
        }
        assert!(matches!(
            component.command_map.get(&0x11),
            Some(Command::NonParam {
                kind: NonParamKind::Guarded,
                ..
            })
        ));
        assert!(matches!(
            component.command_map.get(&0x12),
            Some(Command::NonParam {
                kind: NonParamKind::Sync,
                ..
            })
        ));
    });
}

#[test]
fn record_and_container_carry_their_payload() {
    let src = format!(
        r#"{PORTS}
passive component C {{
  product request port productRequestOut
  sync product recv port productRecvIn
  product send port productSendOut
  time get port timeGetOut

  product container Con id 0x10 default priority 3
  product record Scalar: U32 id 0x20
  product record Arr: U32 array id 0x21
}}
"#
    );
    with_analysis(&src, |a| {
        let component = the_component(a);
        let container = component.container_map.get(&0x10).expect("container");
        assert_eq!(container.get_name(), "Con");
        assert_eq!(container.default_priority, Some(3));

        let scalar = component.record_map.get(&0x20).expect("scalar record");
        assert_eq!(scalar.get_name(), "Scalar");
        assert!(!scalar.is_array);
        assert!(matches!(
            scalar.record_type.as_ref(),
            Type::PrimitiveInt(fpp_ast::IntegerKind::U32)
        ));

        let arr = component.record_map.get(&0x21).expect("array record");
        assert!(arr.is_array);
    });
}

#[test]
fn state_machine_instance_carries_priority_and_queue_full() {
    let src = format!(
        r#"{PORTS}
state machine S {{
  initial enter S1
  state S1
}}

active component C {{
  async input port p: Fw.Cmd

  state machine instance s1: S priority 5 drop
  state machine instance s2: S
}}
"#
    );
    with_analysis(&src, |a| {
        let component = the_component(a);
        let s1 = component
            .state_machine_instance_map
            .get("s1")
            .expect("instance s1");
        assert_eq!(s1.priority, Some(5));
        assert!(matches!(s1.queue_full, QueueFull::Drop));
        let s2 = component
            .state_machine_instance_map
            .get("s2")
            .expect("instance s2");
        assert_eq!(s2.priority, None);
        assert!(matches!(s2.queue_full, QueueFull::Assert));
        assert!(component.has_state_machine_instances());
    });
}

#[test]
fn component_instance_carries_its_attributes() {
    let src = format!(
        r#"{PORTS}
active component C {{
  async input port p: Fw.Cmd
}}

instance c: C base id 0x100 \
  type "CImpl" \
  at "impl/C.hpp" \
  queue size 10 \
  stack size 1024 \
  priority 3 \
  cpu 0
"#
    );
    with_analysis(&src, |a| {
        let instance = the_instance(a);
        assert_eq!(instance.get_unqualified_name(), "c");
        assert_eq!(instance.get_qualified_name(), "c");
        assert_eq!(instance.base_id, 0x100);
        assert_eq!(instance.queue_size, Some(10));
        assert_eq!(instance.stack_size, Some(1024));
        assert_eq!(instance.priority, Some(3));
        assert_eq!(instance.cpu, Some(0));
        // The implementation file is resolved against the directory of the file
        // that specifies it, then normalized.
        assert_eq!(instance.file.as_deref(), Some("impl/C.hpp"));
        // The instance resolves to its component through the analysis.
        assert!(instance.get_component(a).is_some());
        assert!(instance.get_interface(a).is_some());
    });
}

#[test]
fn abstract_type_is_its_own_default_value() {
    let src = r#"
type T
array A = [2] T
struct S { x: T }
"#;
    with_analysis(src, |a| {
        let abs = a
            .symbol_map
            .values()
            .find_map(|s| match s {
                Symbol::AbsType(def) => a.type_map.get(&def.node_id),
                _ => None,
            })
            .expect("the abstract type");
        assert!(matches!(abs.default_value(), Some(Value::AbsType(_))));

        // An aggregate of abstract types therefore has a default value too.
        let array = a
            .symbol_map
            .values()
            .find_map(|s| match s {
                Symbol::Array(def) => a.type_map.get(&def.node_id),
                _ => None,
            })
            .expect("the array type");
        match array.as_ref() {
            Type::Array(array_ty) => {
                let default = array_ty.default.as_ref().expect("an array default value");
                assert_eq!(default.anon_array.elements.len(), 2);
                // Every element is the abstract type's own opaque default, and
                // the repeated element is stored once
                assert!(
                    default
                        .anon_array
                        .iter()
                        .all(|e| matches!(e, Value::AbsType(_)))
                );
                assert!(Arc::ptr_eq(
                    &default.anon_array.elements[0],
                    &default.anon_array.elements[1]
                ));
                // The default value names the array type it belongs to.
                assert_eq!(default.ty.def_node_id(), array.def_node_id());
            }
            other => panic!("expected an array type, got {other:?}"),
        }
    });
}

/// A nested array default holds `size` references to one element value, as
/// Scala's `List.fill(size)(elt)` does, so its cost is the SUM of the nested
/// sizes and not their PRODUCT. Without the sharing, `array Inner = [n] U8` +
/// `array Outer = [n] Inner` peaks at 1.1 GB of resident memory for n=4000 and
/// 4.2 GB for n=8000, against a flat ~160 MB for `fpp-check`, and cannot
/// complete at the top of the legal size range (n = 2^31-1).
#[test]
fn nested_array_default_shares_its_repeated_element() {
    const N: usize = 4000;
    let src = format!("array Inner = [{N}] U8\narray Outer = [{N}] Inner\n");
    with_analysis(&src, |a| {
        let outer = a
            .symbol_map
            .values()
            .find_map(|s| match s {
                Symbol::Array(def) if def.name.data == "Outer" => a.type_map.get(&def.node_id),
                _ => None,
            })
            .expect("the Outer array type");
        let Type::Array(outer_ty) = outer.as_ref() else {
            panic!("expected an array type, got {outer:?}");
        };
        let default = &outer_ty
            .default
            .as_ref()
            .expect("an array default")
            .anon_array;
        assert_eq!(default.elements.len(), N);

        // Every element of Outer's default is the one shared Inner default ...
        let first = &default.elements[0];
        assert!(
            default.elements.iter().all(|e| Arc::ptr_eq(e, first)),
            "Outer's default does not share its repeated element"
        );

        // ... which shares its own repeated U8 zero in turn, so the whole
        // default holds 2N references to 2 distinct values
        let Value::Array(inner) = first.as_ref() else {
            panic!("expected an array element, got {first:?}");
        };
        assert_eq!(inner.anon_array.elements.len(), N);
        let inner_first = &inner.anon_array.elements[0];
        assert!(
            inner
                .anon_array
                .elements
                .iter()
                .all(|e| Arc::ptr_eq(e, inner_first)),
            "Inner's default does not share its repeated element"
        );
        assert!(inner.anon_array.iter().all(|e| e.to_string() == "0"));
    });
}

/// Narrowing a float to an integer element type goes through the width of an
/// `i32`, as Scala's `Double.intValue` does, so a float beyond that range
/// saturates at `i32::MAX` rather than keeping more of the value.
///
/// `fpp-to-json` on this source reports `{"PrimitiveInt": {"value": 2147483647,
/// "kind": {"I32"}}}` and `{"value": 2147483647, "kind": {"I64"}}` for the two
/// defaults.
#[test]
fn float_array_default_narrows_through_i32() {
    let src = r#"
array Wide = [1] I32 default [1.0e300]
array Big = [1] I64 default [1.0e18]
"#;
    with_analysis(src, |a| {
        let element_of = |name: &str| {
            let ty = a
                .symbol_map
                .values()
                .find_map(|s| match s {
                    Symbol::Array(def) if def.name.data == name => a.type_map.get(&def.node_id),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("the {name} array type"));
            let Type::Array(array_ty) = ty.as_ref() else {
                panic!("expected an array type, got {ty:?}");
            };
            array_ty
                .default
                .as_ref()
                .expect("an array default")
                .anon_array
                .get(0)
                .expect("one element")
                .clone()
        };

        assert!(
            matches!(element_of("Wide"), Value::PrimitiveInteger(v)
                if v.value == 2147483647 && v.kind == fpp_ast::IntegerKind::I32),
            "got {:?}",
            element_of("Wide")
        );
        assert!(
            matches!(element_of("Big"), Value::PrimitiveInteger(v)
                if v.value == 2147483647 && v.kind == fpp_ast::IntegerKind::I64),
            "got {:?}",
            element_of("Big")
        );
    });
}

#[test]
fn enum_default_is_an_enum_constant() {
    let src = r#"
enum E { A, B, C }
enum F { A = 1, B = 4, C = 5 } default B
"#;
    with_analysis(src, |a| {
        let mut enums: Vec<_> = a
            .symbol_map
            .values()
            .filter_map(|s| match s {
                Symbol::Enum(def) => Some((def.name.data.clone(), a.type_map.get(&def.node_id)?)),
                _ => None,
            })
            .collect();
        enums.sort_by(|x, y| x.0.cmp(&y.0));
        assert_eq!(enums.len(), 2);
        for (name, ty) in enums {
            match ty.as_ref() {
                Type::Enum(enum_ty) => {
                    let default = enum_ty.default.as_ref().expect("an enum default");
                    let (member, value) = &default.value;
                    match name.as_str() {
                        "E" => {
                            assert_eq!(member, "A");
                            assert_eq!(*value, 0);
                        }
                        "F" => {
                            assert_eq!(member, "B");
                            assert_eq!(*value, 4);
                        }
                        other => panic!("unexpected enum {other}"),
                    }
                }
                other => panic!("expected an enum type, got {other:?}"),
            }
        }
    });
}

#[test]
fn struct_type_default_supplies_missing_members() {
    let src = format!(
        r#"{PORTS}
struct S {{ x: U32, y: U32 }} default {{ x = 5 }}

passive component C {{
  command recv port cmdIn
  command reg port cmdRegOut
  command resp port cmdResponseOut
  param get port prmGetOut
  param set port prmSetOut

  param p: S default {{ y = 1 }} id 0x10 set opcode 0x20 save opcode 0x21
}}
"#
    );
    with_analysis(&src, |a| {
        // The struct type carries a default value for every member: the one it
        // specifies, and the member type's default for the rest.
        let struct_ty = a
            .symbol_map
            .values()
            .find_map(|s| match s {
                Symbol::Struct(def) => a.type_map.get(&def.node_id),
                _ => None,
            })
            .expect("the struct type");
        let default = match struct_ty.as_ref() {
            Type::Struct(ty) => ty.default.as_ref().expect("a struct default"),
            other => panic!("expected a struct type, got {other:?}"),
        };
        assert_eq!(default.anon_struct.members.len(), 2);
        assert!(
            matches!(default.anon_struct.members.get("x"), Some(Value::PrimitiveInteger(v)) if v.value == 5)
        );
        assert!(
            matches!(default.anon_struct.members.get("y"), Some(Value::PrimitiveInteger(v)) if v.value == 0)
        );

        // Converting a partial struct value to the struct type fills the
        // missing members from the struct type's own default, not from the
        // member type's default.
        let param = the_component(a)
            .param_map
            .get(&0x10)
            .expect("param at 0x10");
        let members = match param.default.as_ref().expect("a param default") {
            Value::Struct(v) => &v.anon_struct.members,
            other => panic!("expected a struct value, got {other:?}"),
        };
        assert!(matches!(members.get("x"), Some(Value::PrimitiveInteger(v)) if v.value == 5));
        assert!(matches!(members.get("y"), Some(Value::PrimitiveInteger(v)) if v.value == 1));
    });
}

#[test]
fn symbol_get_loc_is_the_definition_span() {
    let src = "constant a = 1\n";
    with_analysis(src, |a| {
        let symbol = a
            .symbol_map
            .values()
            .find(|s| matches!(s, Symbol::Constant(_)))
            .expect("the constant symbol");
        assert_eq!(symbol.get_loc(), symbol.node().span());
    });
}

/// A shift preserves the primitive-integer kind of its left operand when the shift
/// amount is also a primitive integer, and yields an unsized integer when the
/// amount is an unsized integer.
#[test]
fn shift_preserves_the_left_operand_integer_kind() {
    let src = "\
enum E: U8 { X = 3 }
enum F: U16 { Y = 2 }
constant kindPreserved = E.X << F.Y
constant kindDropped = E.X << 2
constant plainInteger = 1 << 2
";
    with_analysis(src, |a| {
        let value_of = |name: &str| {
            let def = a
                .symbol_map
                .values()
                .find_map(|s| match s {
                    Symbol::Constant(def) if def.name.data == name => Some(def),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("the constant {name}"));
            a.value_map
                .get(&def.value.node_id)
                .unwrap_or_else(|| panic!("a value for {name}"))
        };

        // U8 (the representation type of `E`) survives a U16 shift amount
        assert!(
            matches!(value_of("kindPreserved"), Value::PrimitiveInteger(v)
                if v.value == 12 && v.kind == fpp_ast::IntegerKind::U8),
            "got {:?}",
            value_of("kindPreserved")
        );
        // An unsized shift amount drops the kind
        assert!(
            matches!(value_of("kindDropped"), Value::Integer(v) if v.0 == 12),
            "got {:?}",
            value_of("kindDropped")
        );
        assert!(
            matches!(value_of("plainInteger"), Value::Integer(v) if v.0 == 4),
            "got {:?}",
            value_of("plainInteger")
        );
    });
}
