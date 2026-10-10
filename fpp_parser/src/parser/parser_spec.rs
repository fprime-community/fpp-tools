//! Port of the Scala parser unit tests (`ParserSpec` in
//! `compiler/lib/src/test/scala/syntax/Parser.scala`).
//!
//! Each test parses its inputs with one nonterminal of the grammar, which is why
//! this module lives inside the parser: the nonterminals are private to it.

use super::Parser;
use crate::cursor::Cursor;
use crate::error::ParseResult;
use fpp_core::SourceFile;
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
        let mut parser = Parser {
            cursor: Cursor::new(source_file, s, None),
            include_span: None,
        };
        let result = match p(&mut parser) {
            Ok(_) if parser.peek(0) == TokenKind::EOF => Ok(()),
            Ok(_) => Err(format!("unexpected token {}", parser.peek(0))),
            Err(err) => Err(format!("{err:?}")),
        };
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
fn expression_ok() {
    parse_all_ok(
        |p| p.expr(),
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
fn spec_template_expand_ok() {
    parse_all_ok(
        |p| p.spec_template_expand(),
        &[
            "expand T",
            "expand M.T()",
            "expand M.T(constant 1)",
            "expand M.T(constant 1, constant 2)",
            "expand M.T(constant 1, constant 2, instance mod.$instance)",
            "expand M.T(constant 1, constant 2, instance .mod.$instance, type Type.Name)",
            "expand M.T(constant 1, instance .inst1, type U32)",
            "expand M.T(constant 1, constant 2, constant {member=1}, constant [1, 2])",
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

#[test]
fn tlm_channel_identifier_ok() {
    parse_all_ok(
        |p| p.tlm_channel_identifier(),
        &["a.b", "a.b.c", ".a.b", ".a.b.c"],
    );
}
