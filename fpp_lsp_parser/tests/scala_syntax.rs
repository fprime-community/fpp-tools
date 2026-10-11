//! Accept/reject parity with the Scala parser. Each case records whether
//! Scala `fpp-syntax` accepts the input; fragment entry points are checked
//! against Scala by including the fragment in the construct that the entry
//! point stands for.

use fpp_lsp_parser::{TopEntryPoint, parse};

use TopEntryPoint::*;

const CASES: &[(TopEntryPoint, &str, bool)] = &[
    // Newlines before `)`, `]` and `}` are dropped
    (Module, "constant a = (1\n)\n", true),
    (Module, "constant a = (1 + 2\n)\n", true),
    (Module, "constant a = (1 # comment\n)\n", true),
    (Module, "constant a = sizeof(U32\n)\n", true),
    (Module, "constant a = sizeof(\n  U32\n)\n", true),
    (Module, "array A = [3\n] U32\n", true),
    (Module, "constant a = b[1\n]\n", true),
    (Module, "struct S { x: [3\n] U32 }\n", true),
    (
        Module,
        "passive component C { output port p: [3\n] P }\n",
        true,
    ),
    (
        Module,
        "module M { topology T { connections C { a.b[1\n] -> c.d } } }\n",
        true,
    ),
    (Module, "constant a = (1 << 2\n) >> 3\n", true),
    // `->` absorbs the newlines after it; `<<` and `>>` do not
    (
        Module,
        "module M { topology T { connections C { a.b ->\n c.d } } }\n",
        true,
    ),
    (Module, "port P ->\n  U32\n", true),
    (Module, "constant a = 1 +\n  2\n", true),
    (Module, "constant a = 1 <<\n  2\n", false),
    (Module, "constant a = 4 >>\n  1\n", false),
    // Annotations
    (
        Module,
        "module M {\n  @ pre a\n  constant a = 1 @< post a\n  @ pre b1\n  @ pre b2\n  constant b = 2\n  @< post b\n}\n",
        true,
    ),
    (Module, "@ a\n@ b\nconstant a = 1 @< c\n  @< d\n", true),
    (Module, "constant a = 1\n@< post\n", true),
    (Module, "enum E { A, @ x\n B }\n", true),
    (Module, "port P(\n  @ x\n  a: U32 @< y\n)\n", true),
    (
        Module,
        "instance i: C base id 0 {\n  @ x\n  phase 0 \"code\"\n}\n",
        true,
    ),
    (
        Module,
        "state machine S { choice C { if g enter A else enter B } @< x\n}\n",
        true,
    ),
    (Module, "@ dangling\n", false),
    (
        Module,
        "module M {\n  constant a = 1\n  @ dangling\n}\n",
        false,
    ),
    (
        Module,
        "module M {\n  @< dangling post\n  constant a = 1\n}\n",
        false,
    ),
    (Module, "module M { @< post\n}\n", false),
    (Module, "constant a = @ x\n 1\n", false),
    (Module, "type T = @ x\n U32\n", false),
    (Module, "constant a = [1, @ x\n 2]\n", false),
    (Module, "constant a = 1 + @< x\n 2\n", false),
    (
        Module,
        "constant a = 1 @ pre on same line\nconstant b = 2\n",
        false,
    ),
    (
        Module,
        "module M { topology T { connections C {\n @ x\n a.b -> c.d\n } } }\n",
        false,
    ),
    (
        Module,
        "module M { topology T { telemetry packets P { packet A group 0 {\n @ x\n a.b\n } } } }\n",
        false,
    ),
    (Module, "expand T(\n @ x\n constant 1)\n", false),
    (
        Module,
        "state machine S { state A { entry do {\n @ x\n a } } }\n",
        false,
    ),
    (
        Module,
        "state machine S { choice C {\n @ x\n if g enter A else enter B } }\n",
        false,
    ),
    (
        Module,
        "state machine S { choice C { if g enter A else enter B @< x\n } }\n",
        false,
    ),
    (Module, "enum E { A, @ x\n}\n", false),
    (Module, "port P(a: U32, @ x\n)\n", false),
    // Terminators
    (Module, "constant a = 1; constant b = 2\n", true),
    (Module, "; constant a = 1\n", false),
    (Module, "constant a = 1;;\n", false),
    (Module, "constant a = 1;\n;\n", false),
    (Module, "passive component C { }\n;\n", false),
    (Module, "module M { constant a = 1;; }\n", false),
    (Module, "constant a = 1 constant b = 2\n", false),
    (
        Component,
        "event E severity activity high format \"x\";;\n;\n",
        false,
    ),
    // Element sequences
    (Module, "constant a = [,]\n", true),
    (Module, "constant a = {,}\n", true),
    (Module, "constant a = [\n  0\n  1\n]\n", true),
    (Module, "constant a = [0, 1,]\n", true),
    (Module, "constant a = [1,,]\n", false),
    (Module, "constant a = [, 1]\n", false),
    (Module, "expand T()\n", true),
    (Module, "expand T(,)\n", true),
    (
        Module,
        "state machine S { state A { entry do { , } } }\n",
        true,
    ),
    // Fragments
    (
        Component,
        "dictionary type T = U32\nsync input port p: P\n",
        true,
    ),
    (
        Topology,
        "@ Included instance\ninstance x\n@< Included instance\n",
        true,
    ),
    (TlmPacket, "i5.c5\n", true),
    (TlmPacket, "i5.c5, i5.c6\n", true),
    (TlmPacket, "i5.c5\ni5.c6\n", true),
    (TlmPacket, "include \"channel.fppi\"\n", true),
    (TlmPacket, "# leading comment\ni5.c5\n", false),
    (TlmPacket, "% garbage )))\n", false),
    (TlmPacket, "@ x\ni5.c5\n", false),
    (TlmPacketSet, "packet P group 0 { a.b }\n", true),
    (
        TlmPacketSet,
        "@ Included packet\npacket P2 id 1 group 1 {\n  i4.c4\n  include \"channel.fppi\"\n}\n@< Included packet\n",
        true,
    ),
    (
        TlmPacketSet,
        "packet P group 0 { a.b }, packet Q group 0 { a.b }\n",
        true,
    ),
    (TlmPacketSet, "instance x\n", false),
    (
        StateMachine,
        "state S1 {\n    entry do { a1, a2 }\n}\n\n@ Initial transition\ninitial enter S1\n",
        true,
    ),
    (StateMachine, "entry do { a1 }\n", false),
    (
        State,
        "entry do { a1, a2 }\nexit do { a1, a2 }\n@ Initial transition\ninitial do { a1, a2 } enter S3\n",
        true,
    ),
    (State, "signal s\n", false),
];

#[test]
fn scala_accept_reject() {
    let mut failures = Vec::new();
    for (entry, text, scala_accepts) in CASES {
        let errors = parse(text, *entry).errors();
        if errors.is_empty() != *scala_accepts {
            failures.push(format!(
                "{:?} {:?}: Scala {}, LSP {:?}",
                entry,
                text,
                if *scala_accepts { "accepts" } else { "rejects" },
                errors.iter().map(|e| e.to_string()).collect::<Vec<_>>()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
