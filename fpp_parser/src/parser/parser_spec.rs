//! Port of the Scala parser unit tests (`ParserSpec` in
//! `compiler/lib/src/test/scala/syntax/Parser.scala`).
//!
//! Each test parses its inputs with one nonterminal of the grammar, which is why
//! this module lives inside the parser: the nonterminals are private to it.
//!
//! The Scala spec's "template format round trip OK" case writes an AST back to
//! FPP source with `FppWriter` and compares the two ASTs with `AstWriter`. This
//! port has neither writer, so that case is not ported.

use super::Parser;
use crate::error::ParseResult;
use fpp_core::{Annotated, SourceFile};
use fpp_lexer::TokenKind;

/// A nonterminal of the grammar
type Nonterminal<T> = fn(&mut Parser) -> ParseResult<T>;

/// Parse `s` with `p`, returning the diagnostics if the parse fails, reports an
/// error, or leaves input unconsumed
fn parse_string<T>(p: Nonterminal<T>, s: &str) -> Result<(), String> {
    let mut diagnostics = vec![];
    let mut ctx = fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
    let result = fpp_core::run(&mut ctx, || {
        let source_file = SourceFile::new("input.fpp", s.to_string());
        let mut parser = Parser::new(source_file, s, None);
        let result = match p(&mut parser) {
            Ok(_) if parser.peek(0) == TokenKind::EOF => Ok(()),
            Ok(_) => Err(format!("unexpected token {}", parser.peek(0))),
            Err(err) => Err(format!("{err:?}")),
        };
        if let Some(err) = parser.eof_error.take() {
            fpp_core::Diagnostic::from(err).emit();
        }
        parser.cursor.emit_errors();
        result
    });
    drop(ctx);

    match (result, diagnostics.is_empty()) {
        (Ok(()), true) => Ok(()),
        (result, _) => Err(format!(
            "{}{}",
            result.err().unwrap_or_default(),
            String::from_utf8_lossy(&diagnostics)
        )),
    }
}

fn parse_all_ok<T>(p: Nonterminal<T>, ss: &[&str]) {
    for s in ss {
        if let Err(err) = parse_string(p, s) {
            panic!("could not parse {s}: {err}");
        }
    }
}

fn parse_all_error<T>(p: Nonterminal<T>, ss: &[&str]) {
    for s in ss {
        assert!(parse_string(p, s).is_err(), "parsed {s}");
    }
}

/// The translation unit nonterminal, which reports its errors as it goes
fn trans_unit(p: &mut Parser) -> ParseResult<fpp_ast::TransUnit> {
    Ok(p.trans_unit())
}

#[test]
fn annotation_ok() {
    let s = "

      @ Line 1

      @ Line 2

      active component C {} @< Line 3

                            @< Line 4

      ";
    let mut diagnostics = vec![];
    let mut ctx = fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
    let (pre, post) = fpp_core::run(&mut ctx, || {
        let source_file = SourceFile::new("input.fpp", s.to_string());
        let tu = super::parse(source_file, |p| p.trans_unit(), None);
        assert_eq!(tu.0.len(), 1);
        (tu.0[0].pre_annotation(), tu.0[0].post_annotation())
    });
    drop(ctx);
    assert!(diagnostics.is_empty());
    assert_eq!(pre, vec!["Line 1", "Line 2"]);
    assert_eq!(post, vec!["Line 3", "Line 4"]);
}

#[test]
fn comment_ok() {
    parse_all_ok(trans_unit, &["# This is a comment"]);
}

#[test]
fn connection_ok() {
    parse_all_ok(|p| p.connection(), &["a.b -> c.d", "a.b.c -> d.e.f"]);
}

#[test]
fn def_abs_type_ok() {
    parse_all_ok(|p| p.def_abs_type(), &["type t"]);
}

#[test]
fn def_alias_type_ok() {
    parse_all_ok(
        |p| p.def_alias_type(),
        &["type a = U32", "type a = F32", "dictionary type a = string"],
    );
}

#[test]
fn def_array_ok() {
    parse_all_ok(
        |p| p.def_array(),
        &[
            "array a = [10] U32",
            "array a = [10] U32 default 0",
            "array a = [10] U32 default 0 format \"{} counts\"",
            "dictionary array a = [10] U32",
        ],
    );
}

#[test]
fn def_component_ok() {
    parse_all_ok(
        |p| p.def_component(),
        &[
            "active component C { }",
            "passive component C { }",
            "queued component C { }",
            "active component C {
          constant a = 0
          struct S { x: U32 }
          async command C
          param p: U32
          async input port p: P
          telemetry T: U32
          array A = [10] U32
          enum E { X = 0, Y = 1 }
          event E severity activity low format \"Event E\"
          include \"a.fpp\"
          internal port P
          state machine S
          state machine instance s1: S
          state machine instance s2: S
        }",
            "active component C {
          @ Pre
          constant a = 0 @< Post
          @ Pre
          constant b = 0 @< Post
        }",
        ],
    );
}

#[test]
fn def_component_instance_ok() {
    parse_all_ok(
        |p| p.def_component_instance(),
        &[
            "instance i: C base id 0x100",
            "instance i: C base id 0x100 type \"T\"",
            "instance i: C base id 0x100 type \"T\" at \"file.hpp\"",
            "instance i: C base id 0x100 type \"T\" at \"file.hpp\" queue size 10",
            "instance i: C base id 0x100 type \"T\" at \"file.hpp\" queue size 10 stack size 1024",
            "instance i: C base id 0x100 type \"T\" at \"file.hpp\" queue size 10 stack size 1024 priority 3",
            "instance i: C base id 0x100 type \"T\" at \"file.hpp\" queue size 10 stack size 1024 priority 3 cpu 0",
            "instance i: C base id 0x100 type \"T\" at \"file.hpp\" queue size 10 stack size 1024 priority 3 cpu 0 { phase 0 \"code\" }",
        ],
    );
}

// Rust-only: the base id is required
#[test]
fn def_component_instance_error() {
    parse_all_error(
        |p| p.def_component_instance(),
        &[
            "instance i: C",
            "instance i: C type \"T\"",
            "instance i: C base 0x100",
        ],
    );
}

#[test]
fn def_constant_ok() {
    parse_all_ok(
        |p| p.def_constant(),
        &[
            "constant a = 0",
            "dictionary constant a = 0",
            "constant a = sizeof(bool)",
        ],
    );
}

#[test]
fn def_enum_ok() {
    parse_all_ok(
        |p| p.def_enum(),
        &[
            "enum E { X, Y }",
            "enum E { X = 0, Y = 1 }",
            "enum E {
          @ Pre
          X = 0 @< Post
          @ Pre
          Y = 1 @< Post
        }",
            "dictionary enum E { X = 0 }",
        ],
    );
}

#[test]
fn def_module_ok() {
    parse_all_ok(
        |p| p.def_module(),
        &[
            "module M {}",
            "module M {
          active component C {}
          instance i: C base id 0x100
          constant a = 0
          module M {}
          port P
          struct S {}
          topology T {}
          locate component C at \"c.fpp\"
          type T
          type K = T
          array A = [10] U32
          enum E { X, Y }
          include \"a.fpp\"
        }",
            "module M {
          @ Pre
          active component C {} @< Post
        }",
        ],
    );
}

#[test]
fn def_port_ok() {
    parse_all_ok(
        |p| p.def_port(),
        &[
            "port P",
            "port P()",
            "port P(x: U32)",
            "port P(x: U32) -> U32",
        ],
    );
}

#[test]
fn def_state_machine_ok() {
    parse_all_ok(
        |p| p.def_state_machine(),
        &[
            "state machine M",
            "state machine M {
          constant c = 0
          type T
          type X = T
          array A = [3] U32
          struct S { x: U32 }
          enum E { A, B }
          action a
          signal s
          guard g
          initial enter S
          state S {
            on g enter T
          }
          state T
        }",
        ],
    );
}

#[test]
fn def_struct_ok() {
    parse_all_ok(
        |p| p.def_struct(),
        &[
            "struct S {}",
            "struct S { x: U32, y: F32 }",
            "struct S { x: U32 format \"{} steps\", y: F32 format \"{} m/s\" }",
            "struct S { x: U32, y: F32 } default { x = 1, y = 2 }",
            "struct S { x: [3] U32 }",
            "struct S {
          @ Pre
          x: U32 @< Post
          @ Pre
          y: F32 @< Post
        }",
            "dictionary struct S { x: [3] U32 }",
        ],
    );
}

#[test]
fn def_system_ok() {
    parse_all_ok(
        |p| p.def_system(),
        &["system S: T", "system S: M.T", "system S: M1.M2.T"],
    );
}

#[test]
fn def_tlm_packet_ok() {
    parse_all_ok(
        |p| p.spec_tlm_packet(),
        &[
            "packet P group 0 {
             include \"file.fpp\"
             a.b
             c.d
        }",
            "packet P id 0 group 0 {}",
        ],
    );
}

#[test]
fn def_tlm_packet_set_ok() {
    parse_all_ok(
        |p| p.spec_tlm_packet_set(),
        &[
            "telemetry packets P {
             @ P1
             packet P1 group 0 {
               include \"file.fpp\"
               a.b
               c.d
             }
             packet P2 id 1 group 0 {}
           }",
            "telemetry packets P {} omit {}",
            "telemetry packets P {} omit { a.b, c.d }",
        ],
    );
}

#[test]
fn def_topology_ok() {
    parse_all_ok(
        |p| p.def_topology(),
        &[
            "topology T {}",
            "deployment topology T {}",
            "topology T implements I {}",
            "topology T implements I1.I2 {}",
            "topology T implements I1.I2, I3.I4 {}",
            "topology T {
          instance i
          connections C {}
          import T
          port p = a.b
          include \"a.fpp\"
        }",
            "topology T {
          @ Pre
          instance i @< Post
        }",
            "topology T {
          telemetry packets P {}
        }",
        ],
    );
}

#[test]
fn expression_ok() {
    parse_all_ok(
        |p| p.expr_node(),
        &[
            "-1",
            "1+1",
            "1-1",
            "1*1",
            "1/1",
            "[0, 1, 2]",
            "[
             0
             1
             2
           ]",
            "[
             0,
             1,
             2
           ]",
            "[
             0,
             1,
             2,
           ]",
            "true",
            "false",
            "a.b",
            "a.b.c",
            ".a",
            ".a.b",
            ".a.b.c",
            "1.0",
            "a",
            "1",
            "\"abc\"",
            "{ x = 1, y = 2, z = 3 }",
            "{
             x = 1
             y = 2
             z = 3
           }",
            "{
             x = 1,
             y = 2,
             z = 3
           }",
            "{
             x = 1,
             y = 2,
             z = 3,
           }",
            "sizeof(U32)",
            "sizeof(string size 10)",
            "sizeof(a.b.c)",
        ],
    );
}

#[test]
fn expression_error() {
    parse_all_error(|p| p.expr_node(), &["a.", "bool"]);
}

#[test]
fn expression_error_annotation() {
    parse_all_error(|p| p.expr_node(), &["@"]);
}

#[test]
fn formal_param_list_ok() {
    parse_all_ok(
        |p| p.formal_param_list(),
        &[
            "",
            "()",
            "(x: U32)",
            "(x: U32, y: F64)",
            "(
             x: U32
             y: F64
           )",
            "(
             @ Pre
             x: U32 @< Post
             @ Pre
             y: F64 @< Post
           )",
            "(
             x: U32,
             y: F64
           )",
            "(
             @ Pre
             x: U32, @< Post
             @ Pre
             y: F64 @< Post
           )",
            "(
             x: U32,
             y: F64,
           )",
            "(
             @ Pre
             x: U32, @< Post
             @ Pre
             y: F64, @< Post
           )",
            "(ref x: string)",
            "(ref x: string size 256)",
        ],
    );
}

#[test]
fn port_instance_identifier_ok() {
    parse_all_ok(
        |p| p.port_instance_identifier(),
        &["a.b", "a.b.c", ".a.b", ".a.b.c"],
    );
}

#[test]
fn qual_ident_ok() {
    parse_all_ok(
        |p| p.qual_ident(),
        &["a", "a.b", "a.b.c", ".a", ".a.b", ".a.b.c"],
    );
}

#[test]
fn spec_command_ok() {
    parse_all_ok(
        |p| p.spec_command(),
        &[
            "async command C",
            "guarded command C",
            "sync command C",
            "async command C()",
            "async command C(x: U32)",
            "async command C(x: U32) opcode 0x100",
            "async command C(x: U32) opcode 0x100 priority 10",
            "async command C assert",
            "async command C block",
            "async command C drop",
        ],
    );
}

#[test]
fn spec_instance_ok() {
    parse_all_ok(
        |p| p.spec_instance(),
        &["instance a", "instance a.b", "instance a.b.c"],
    );
}

#[test]
fn spec_connection_graph_direct_ok() {
    parse_all_ok(
        |p| p.spec_connection_graph(),
        &[
            "connections C {}",
            "connections C { a.b -> c.d }",
            "connections C { a.b -> c.d, e.f -> g.h }",
        ],
    );
}

#[test]
fn spec_connection_graph_pattern_ok() {
    parse_all_ok(
        |p| p.spec_connection_graph(),
        &[
            "command connections instance a.b",
            "command connections instance a.b {}",
            "command connections instance a.b { c.d }",
        ],
    );
}

#[test]
fn spec_container_ok() {
    parse_all_ok(
        |p| p.spec_container(),
        &[
            "product container C",
            "product container C id 0x100",
            "product container C id 0x100 default priority 10",
        ],
    );
}

#[test]
fn spec_event_ok() {
    parse_all_ok(
        |p| p.spec_event(),
        &[
            "event E severity activity high format \"event E\"",
            "event E severity activity low format \"event E\"",
            "event E severity command format \"event E\"",
            "event E severity diagnostic format \"event E\"",
            "event E severity fatal format \"event E\"",
            "event E severity warning high format \"event E\"",
            "event E severity warning low format \"event E\"",
            "event E () severity activity high format \"event E\"",
            "event E (x: U32) severity activity high format \"x={}\"",
            "event E severity activity high id 0x100 format \"event E\"",
            "event E (x: U32) severity activity high id 0x100 format \"x={}\"",
            "event E (x: U32) severity activity high id 0x100 format \"x={}\" throttle 10",
            "event E (x: U32) severity activity high id 0x100 format \"x={}\" throttle 10 every {seconds=10,useconds=11}",
        ],
    );
}

#[test]
fn spec_template_expand_ok() {
    parse_all_ok(
        |p| p.spec_template_expand(),
        &[
            "expand T",
            "expand T { constant a = 0 }",
            "expand M.T()",
            "expand M.T() { constant a = 0 }",
            "expand M.T(constant 1)",
            "expand M.T(constant 1) { constant a = 0 }",
            "expand M.T(constant 1, constant 2)",
            "expand M.T(constant 1, constant 2) { constant a = 0 }",
            "expand M.T(constant 1, constant 2, instance mod.$instance)",
            "expand M.T(constant 1, constant 2, instance mod.$instance) { constant a = 0 }",
            "expand M.T(constant 1, constant 2, instance .mod.$instance, type Type.Name)",
            "expand M.T(constant 1, constant 2, instance .mod.$instance, type Type.Name) { constant a = 0 }",
            "expand M.T(constant 1, instance .inst1, type U32)",
            "expand M.T(constant 1, instance .inst1, type U32) { constant a = 0 }",
            "expand M.T(constant 1, constant 2, constant {member=1}, constant [1, 2])",
            "expand M.T(constant 1, constant 2, constant {member=1}, constant [1, 2]) { constant a = 0 }",
        ],
    );
}

#[test]
fn spec_include_ok() {
    parse_all_ok(|p| p.spec_include(), &["include \"file.fpp\""]);
}

#[test]
fn spec_internal_port_ok() {
    parse_all_ok(
        |p| p.spec_internal_port(),
        &[
            "internal port P",
            "internal port P()",
            "internal port P(x: U32)",
            "internal port P(x: U32) priority 10",
            "internal port P(x: U32) priority 10 assert",
        ],
    );
}

#[test]
fn spec_location_ok() {
    parse_all_ok(
        |p| p.spec_loc(),
        &[
            "locate component a.b at \"c.fpp\"",
            "locate component .a.b at \"c.fpp\"",
            "locate constant a.b at \"c.fpp\"",
            "locate constant .a.b at \"c.fpp\"",
            "locate instance a.b at \"c.fpp\"",
            "locate port a.b at \"c.fpp\"",
            "locate state machine a.b at \"c.fpp\"",
            "locate system a.b at \"c.fpp\"",
            "locate type a.b at \"c.fpp\"",
            "locate dictionary type a.b at \"c.fpp\"",
            "locate dictionary constant a.b at \"c.fpp\"",
            "locate template a.b at \"c.fpp\"",
            "locate template $template at \"c.fpp\"",
        ],
    );
}

// Rust-only: only a constant or a type location may be a dictionary location
#[test]
fn spec_location_error() {
    parse_all_error(
        |p| p.spec_loc(),
        &[
            "locate dictionary component a.b at \"c.fpp\"",
            "locate dictionary instance a.b at \"c.fpp\"",
            "locate dictionary state machine a.b at \"c.fpp\"",
            "locate foo a.b at \"c.fpp\"",
        ],
    );
}

#[test]
fn spec_param_ok() {
    parse_all_ok(
        |p| p.spec_param(),
        &[
            "param P: U32",
            "param P: U32 default 0",
            "param P: U32 default 0 id 0x100",
            "param P: U32 default 0 id 0x100 set opcode 0x00",
            "param P: U32 default 0 id 0x100 set opcode 0x00 save opcode 0x01",
        ],
    );
}

#[test]
fn spec_port_instance_general_ok() {
    parse_all_ok(
        |p| p.spec_port_instance(),
        &[
            "async input port p: a",
            "async input port p: a.b",
            "async input port p: serial",
            "guarded input port p: a",
            "output port p: T",
            "sync input port p: T",
            "async input port p: [10] T",
            "async input port p: [10] T priority 10",
            "async input port p: [10] T priority 10 assert",
        ],
    );
}

#[test]
fn spec_port_instance_special_ok() {
    parse_all_ok(
        |p| p.spec_port_instance(),
        &[
            "async product recv port p",
            "command recv port p",
            "command reg port p",
            "command resp port p",
            "event port p",
            "guarded product recv port p",
            "param get port p",
            "param set port p",
            "product get port p",
            "product recv port p",
            "product request port p",
            "product send port p",
            "sync product recv port p",
            "telemetry port p",
            "text event port p",
            "time get port p",
        ],
    );
}

#[test]
fn spec_port_matching_ok() {
    parse_all_ok(|p| p.spec_port_matching(), &["match p1 with p2"]);
}

#[test]
fn spec_record_ok() {
    parse_all_ok(
        |p| p.spec_record(),
        &[
            "product record R: U32",
            "product record R: U32 array",
            "product record R: U32 id 0x100",
        ],
    );
}

#[test]
fn spec_state_machine_instance_ok() {
    parse_all_ok(
        |p| p.spec_state_machine_instance(),
        &["state machine instance s: S"],
    );
}

#[test]
fn spec_tlm_channel_ok() {
    parse_all_ok(
        |p| p.spec_tlm_channel(),
        &[
            "telemetry C: U32",
            "telemetry C: U32 id 0x00",
            "telemetry C: U32 id 0x00 update always",
            "telemetry C: U32 id 0x00 update on change",
            "telemetry C: U32 id 0x00 \\
             update on change \\
             format \"{} counts\"",
            "telemetry C: U32 id 0x00 \\
             update on change \\
             format \"{} counts\" \\
             low {}",
            "telemetry C: U32 id 0x00 \\
             update on change \\
             format \"{} counts\" \\
             low { red 0 }",
            "telemetry C: U32 id 0x00 \\
             update on change \\
             format \"{} counts\" \\
             low { red 0, orange 0 }",
            "telemetry C: U32 id 0x00 \\
             update on change \\
             format \"{} counts\" \\
             low { red 0, orange 0, yellow 0 }",
            "telemetry C: U32 id 0x00 \\
             update on change \\
             format \"{} counts\" \\
             low {} \\
             high {}",
            "telemetry C: U32 id 0x00 \\
             update on change \\
             format \"{} counts\" \\
             low { red 0 } \\
             high { red 0 }",
            "telemetry C: U32 id 0x00 \\
             update on change \\
             format \"{} counts\" \\
             low { red 0, orange 0 } \\
             high { red 0, orange 0 }",
            "telemetry C: U32 id 0x00 \\
             update on change \\
             format \"{} counts\" \\
             low { red 0, orange 0, yellow 0 } \\
             high { red 0, orange 0, yellow 0 }",
        ],
    );
}

#[test]
fn spec_import_ok() {
    parse_all_ok(|p| p.spec_import(), &["import a", "import a.b"]);
}

#[test]
fn spec_topology_port_ok() {
    parse_all_ok(|p| p.spec_top_port(), &["port a = a.b.c.d", "port b = c.b"]);
}

#[test]
fn tlm_channel_identifier_ok() {
    parse_all_ok(
        |p| p.tlm_channel_identifier(),
        &["a.b", "a.b.c", ".a.b", ".a.b.c"],
    );
}

#[test]
fn type_name_ok() {
    parse_all_ok(
        |p| p.type_name(),
        &[
            "I8", "I16", "I32", "I64", "U8", "U16", "U32", "U64", "F32", "F64", "bool", "string",
            "a", "a.b", "a.b.c",
        ],
    );
}

#[test]
fn template_ok() {
    parse_all_ok(
        trans_unit,
        &[
            "module template T {}",
            "module template T() {}",
            "module template T(constant c: U32, instance i: I, type Ty) {}",
            "
        module template T(
          constant c: U32,
          type Ty,
          instance i: Interface
        ) {
        }
        ",
            "
        module template T(
          constant c: U32,
          type Ty,
          instance i: Interface
        ) {
          array a = [3] Ty
        }
        ",
            "
        module template T(
          constant c: U32
          instance i: Interface
          type Ty
        ) {
        }
        ",
            "
        module template T(
          constant c: U32,
          instance i: Interface,
          type Ty
        ) {
          instance inst: Comp base id 0x100
          topology Top {
            instance i
            instance inst
          }
        }
        expand T(constant 1, instance inst1, type U32)
        ",
            // Newline-separated argument list with no commas
            "
        expand T(
          constant 1
          instance inst1
          type U32
        )
        ",
            // An expansion with an empty argument list. A template argument list is
            // an element sequence, and an element sequence may be empty, so this is
            // legal and means the same thing as 'expand T'.
            "expand T()",
            // Annotations on template parameters
            "
        module template T(
          @ Constant parameter
          constant c: U32 @< Constant parameter
          @ Instance parameter
          instance i: Interface @< Instance parameter
          @ Type parameter
          type Ty @< Type parameter
        ) {
        }
        ",
            // A trailing comma on the last element of a parameter list or an
            // argument list. An element sequence permits terminating punctuation on
            // its last element: see docs/spec/Element-Sequences.adoc.
            "
        module template T(
          constant c: U32,
          instance i: Interface,
          type Ty,
        ) {
        }
        expand T(
          constant 1,
          instance inst1,
          type U32,
        )
        ",
            // Escaped reserved words as the names of a template, its parameters,
            // and the definitions in its body
            "
        module template $template(
          constant $constant: U32
          instance $instance: Interface
          type $type
        ) {
          constant $expand = $constant
        }
        expand $template(constant 1, instance inst1, type U32)
        ",
            // A qualified template name
            "expand A.B.T(constant 1)",
        ],
    );
}

#[test]
fn template_error() {
    parse_all_error(
        trans_unit,
        &[
            // A template argument must begin with 'constant', 'instance', or
            // 'type'. Omitting the prefix appeared in the specification's own
            // worked example.
            "expand T(1)",
            "expand T(U32)",
            "expand T(constant 1, 2)",
            // A constant template argument must have an expression
            "expand T(constant)",
            // An expansion must name a template
            "expand",
            // A template parameter must begin with 'constant', 'instance', or
            // 'type'
            "module template T(c: U32) {}",
            // A constant template parameter must have a type
            "module template T(constant c) {}",
            // An instance template parameter must have an interface
            "module template T(instance i) {}",
            // A type template parameter may not have a type
            "module template T(type Ty: U32) {}",
            // A template definition must have an identifier
            "module template {}",
            // A template definition must have a body
            "module template T",
            // A template location specifier must have a name and a file
            "locate template at \"c.fpp\"",
            "locate template T",
        ],
    );
}

#[test]
fn trans_unit_ok() {
    parse_all_ok(
        trans_unit,
        &[
            "",
            "
        # Some newlines


        ",
            "
        active component C {}
        instance i: C base id 0x100
        constant a = 0
        module M {}
        module template TT () {}
        port P
        struct S {}
        topology T {}
        locate component C at \"c.fpp\"
        type T
        array A = [10] U32
        enum E { X, Y }
        expand TT (constant 1, constant 2, constant 3, constant {member=[1, 2, 3]}, type TypeName, instance i)
        include \"a.fpp\"
        ",
            "
        @ Pre
        active component C {} @< Post
        ",
            "


        constant a = 0


        constant b = 1


        ",
        ],
    );
}

#[test]
fn trans_unit_error() {
    parse_all_error(trans_unit, &["$", "abcd", "1+1"]);
}

// Rust-only: a translation unit must consume all of its input, and
// pre-annotations commit to an element
#[test]
fn trans_unit_all_input_error() {
    parse_all_error(
        trans_unit,
        &[
            "constant a = 1\n}\nconstant b = 2\n",
            "module M { constant a = 1 } }\n",
            "module M {\n  @ dangling\n}\n",
            "constant a = 1\n@ dangling\n",
        ],
    );
}

/// The line and column of the first error that parsing `s` as a translation
/// unit reports, if it reports one
fn first_error_location(s: &str) -> Option<(u32, u32)> {
    first_error_location_in(s, |p| {
        p.trans_unit();
    })
}

/// An entry point of the grammar
type EntryPoint = fn(&mut Parser);

/// The line and column of the first error that parsing all of `s` with
/// `entry` reports, if it reports one
fn first_error_location_in(s: &str, entry: EntryPoint) -> Option<(u32, u32)> {
    let mut diagnostics = vec![];
    let mut ctx = fpp_core::CompilerContext::new(fpp_errors::WriteEmitter::new(&mut diagnostics));
    fpp_core::run(&mut ctx, || {
        let source_file = SourceFile::new("input.fpp", s.to_string());
        super::parse(source_file, entry, None);
    });
    drop(ctx);

    let diagnostics = String::from_utf8_lossy(&diagnostics).into_owned();
    let location = diagnostics
        .lines()
        .find_map(|l| l.trim().strip_prefix("--> "))?;
    let mut parts = location.rsplit(':');
    let column = parts.next()?.parse().ok()?;
    let line = parts.next()?.parse().ok()?;
    Some((line, column))
}

// Rust-only: when the input ends inside a member, the first error is where
// the Scala parser reports it, at the furthest failure of the alternatives
// of the member
#[test]
fn trans_unit_end_of_input_error() {
    let cases = [
        // A module template definition fails at the name of a module
        ("@ M\nmodule M {", (2, 8)),
        // A module definition expects a name where `template` is
        ("module template T(", (1, 8)),
        ("passive component C {\n  param get port", (2, 9)),
        // The members that share the keyword of the member fail after it
        (
            "passive component C {\n  @ c\n  product container C id",
            (3, 11),
        ),
        (
            "passive component C {\n  @ e\n  event E(a: U32) severity",
            (3, 9),
        ),
        (
            "passive component C {\n  @ t\n  telemetry T: U32 id",
            (3, 13),
        ),
        ("passive component C {\n  @ c\n  async command", (3, 9)),
        (
            "passive component C {\n  @ c\n  async command C(a: U32",
            (3, 17),
        ),
        (
            "passive component C {\n  @ p\n  sync input port p: T priority",
            (3, 8),
        ),
        ("passive component C {\n  @ s\n  state machine S {", (3, 17)),
        (
            "passive component C {\n  @ s\n  state machine instance s: S priority",
            (3, 17),
        ),
        (
            "passive component C {\n  @ p\n  param p: U32 default",
            (3, 9),
        ),
        ("topology T {\n  @ p\n  telemetry packets P {", (3, 13)),
        (
            "topology T {\n  @ p\n  telemetry connections instance a {",
            (3, 13),
        ),
        ("@ d\ndictionary constant c = 1 +", (2, 12)),
        (
            "passive component C {\n  @ p\n  async product recv",
            (3, 17),
        ),
        ("passive component C {\n  @ p\n  command reg", (3, 11)),
        // Without a pre-annotation, the member ends the sequence before it
        ("module M {\n  module N {", (2, 3)),
    ];
    for (s, location) in cases {
        assert_eq!(first_error_location(s), Some(location), "{s}");
    }
}

// Rust-only: when the input ends inside an expression, a type name or
// another construct with alternatives of its own, the first error is the
// furthest failure of the alternatives of the construct
#[test]
fn trans_unit_end_of_input_construct_error() {
    let cases = [
        // An array is the first alternative of a primary expression, and a
        // unary minus fails at the start of the operand
        ("module M {\n  constant a = [1, 2", (2, 16)),
        // The other primary expressions fail at their start
        ("module M {\n  constant a = (", (2, 16)),
        ("module M {\n  constant a = {x = 1", (2, 16)),
        ("module M {\n  constant a = .", (2, 16)),
        // A parenthesized expression or a size fails where the expression
        // or the type name inside it last fails
        ("module M {\n  constant a = (1 + 2", (2, 21)),
        ("module M {\n  constant a = sizeof(F64", (2, 23)),
        ("module M {\n  constant a = sizeof(string size 2", (2, 35)),
        // A subscript is not a selector, so the expression ends before it
        ("module M {\n  constant a = b[1", (2, 17)),
        // A string type with a size is not a type name
        ("module M {\n  array A = [3] string size 1 +", (2, 17)),
        // A transition fails where its actions do
        ("state machine S {\n  choice C { if g enter", (2, 19)),
        ("state machine S {\n  @ i\n  initial do { a }", (3, 18)),
        (
            "state machine S {\n  state A {\n    on s if g enter",
            (3, 15),
        ),
        // The other severity levels and update kinds fail at the first
        // keyword
        (
            "passive component C {\n  event E severity activity",
            (2, 20),
        ),
        (
            "passive component C {\n  telemetry T: U32 update on",
            (2, 27),
        ),
        // A member selection or qualifier fails after its dot
        ("constant a = (a.)\n", (1, 17)),
        ("constant a = sizeof(A.)\n", (1, 23)),
    ];
    for (s, location) in cases {
        assert_eq!(first_error_location(s), Some(location), "{s}");
    }
}

// Rust-only: a subscript that fails before its commit point, or at the end
// of the input, is not a selector, so the expression ends before it, and
// what must follow the expression fails at the bracket, or where the
// subscript fails if that does not commit
#[test]
fn trans_unit_subscript_error() {
    let cases = [
        ("module M {\n  constant a = b[ * 3\n}\n", (2, 17)),
        ("module M {\n  constant a = (b[ * 3\n}\n", (2, 20)),
        ("constant a = (b[)\n", (1, 17)),
        ("constant a = a[b[ * 3]\n", (1, 17)),
        ("array A = [b[ * 3] U32\n", (1, 13)),
        ("constant a = (b[1", (1, 16)),
        ("constant a = a[b[1", (1, 17)),
        ("array A = [b[1", (1, 13)),
        ("constant a = sizeof(string size b[1", (1, 34)),
        ("module M {\n  constant a = a[b[1", (2, 19)),
        ("instance i: C base id 0 {\n  phase b[1", (2, 10)),
        (
            "module M {\n  passive component C {\n    event E severity fatal id b[1",
            (3, 32),
        ),
    ];
    for (s, location) in cases {
        assert_eq!(first_error_location(s), Some(location), "{s}");
    }

    // Files of members, such as an included file holds
    let cases: [(&str, EntryPoint, (u32, u32)); 3] = [
        (
            "event E severity fatal id b[1",
            |p| {
                p.component_members();
            },
            (1, 28),
        ),
        (
            "packet A id b[1",
            |p| {
                p.tlm_packet_set_members();
            },
            (1, 14),
        ),
        (
            "packet A group b[ * 3\n",
            |p| {
                p.tlm_packet_set_members();
            },
            (1, 17),
        ),
    ];
    for (s, entry, location) in cases {
        assert_eq!(first_error_location_in(s, entry), Some(location), "{s}");
    }
}

#[test]
fn type_name_error() {
    parse_all_error(|p| p.type_name(), &["a.", "1+3"]);
}

#[test]
fn type_name_error_annotation() {
    parse_all_error(|p| p.type_name(), &["@"]);
}
