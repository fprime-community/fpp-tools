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
    // A line continuation ends a run of newlines
    (Module, "constant a = (1\n\\\n)\n", false),
    (Module, "constant a = (1 # comment\n\\\n)\n", false),
    (Module, "constant a = (1 \\\n)\n", true),
    (Module, "constant a = (1 \\\n\n)\n", true),
    (Module, "constant a = [1\n\\\n]\n", true),
    (Module, "constant a = (\\\n1)\n", true),
    (Module, "constant a = (\\\n\n1)\n", false),
    (Module, "constant a = 1 +\n\\\n  2\n", true),
    (Module, "constant a = 1 + \\\n\n  2\n", false),
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
    // Absolute names
    (
        Module,
        "module M {\n  constant a = 1\n  constant b = .a\n  constant c = .M.a\n  type T = .M.U\n  type U = U32\n}\n",
        true,
    ),
    (
        Module,
        "module M { topology T { instance .M.i\n port p = .M.i.out\n connections C { .M.i.out -> .M.j.in }\n telemetry packets P { packet A group 0 { .M.i.ch } } omit { .M.i.ch2 } } }\n",
        true,
    ),
    (
        Module,
        "locate constant .M.c at \"c.fpp\"\nexpand .M.T(type .M.U, instance .M.i, constant .M.c)\n",
        true,
    ),
    (
        Module,
        "state machine S { initial enter .S.A\n state A }\n",
        true,
    ),
    (
        Module,
        "passive component C { sync input port p: .M.P\n state machine instance s: .M.S }\n",
        true,
    ),
    (Module, "topology T implements .I { import .A }\n", true),
    (Module, "system S: .T\n", true),
    (Module, "module template T(instance i: .I) { }\n", true),
    (Module, "constant a = . a\n", true),
    (Module, "constant a = .5\n", true),
    (Module, "constant a = .\n", false),
    (Module, "type T = .\n", false),
    (Module, "constant a = 1 .x\n", true),
    (TlmPacket, ".i5.c5\n", true),
    // Port instance and channel identifiers have at least two names
    (Module, "module M { topology T { port p = a } }\n", false),
    (
        Module,
        "module M { topology T { connections C { a -> b } } }\n",
        false,
    ),
    (
        Module,
        "module M { topology T { telemetry packets P { packet A group 0 { a } } } }\n",
        false,
    ),
    (
        Module,
        "module M { topology T { telemetry packets P { packet A group 0 { a.b } } omit { c } } }\n",
        false,
    ),
    // Implements lists are element sequences
    (Module, "topology T implements {}\n", true),
    (Module, "topology T implements , {}\n", true),
    (Module, "topology T implements A\nB {}\n", true),
    (Module, "topology T implements I\n{ }\n", true),
    (Module, "topology T implements I, J, { }\n", true),
    (Module, "topology T implements I,\n J { }\n", true),
    (Module, "topology T implements I J { }\n", false),
    // Dictionary definitions
    (
        Module,
        "passive component C {\n  dictionary type T = U32\n  dictionary constant c = 1\n}\n",
        true,
    ),
    (
        Module,
        "passive component C { dictionary event E severity activity high format \"x\" }\n",
        false,
    ),
    (
        Module,
        "passive component C { dictionary sync command C }\n",
        false,
    ),
    (
        Module,
        "passive component C { dictionary dictionary constant c = 1 }\n",
        false,
    ),
    (
        Module,
        "passive component C { dictionary include \"x.fppi\" }\n",
        false,
    ),
    (Module, "dictionary type T\n", false),
    (Module, "module M { dictionary type T }\n", false),
    (
        Module,
        "locate dictionary constant c at \"c.fpp\"\nlocate dictionary type T at \"t.fpp\"\n",
        true,
    ),
    (
        Module,
        "locate dictionary component C at \"c.fpp\"\n",
        false,
    ),
    (Module, "locate foo\n", false),
    (
        Module,
        "state machine S { dictionary type T = U32\n dictionary constant c = 1\n dictionary array A = [2] U32\n dictionary enum E { X }\n dictionary struct S { x: U32 } }\n",
        true,
    ),
    (
        Module,
        "state machine S { state A { dictionary constant c = 1 } }\n",
        false,
    ),
    // Special port instances with an input kind
    (
        Module,
        "passive component C { sync command recv port cmdIn\n async command reg port r\n guarded command resp port s }\n",
        true,
    ),
    (Module, "interface I { sync command recv port c }\n", true),
    (Module, "passive component C { sync command C }\n", true),
    (
        Module,
        "passive component C { command foo port p }\n",
        false,
    ),
    // Missing names and bodies
    (Module, "module\nconstant a = 1\n", false),
    (Module, "module template\nconstant a = 1\n", false),
    (Module, "state machine\nconstant a = 1\n", false),
    (Module, "port\nconstant a = 1\n", false),
    (Module, "instance\nconstant a = 1\n", false),
    (Module, "topology\nconstant a = 1\n", false),
    (Module, "passive component\nconstant a = 1\n", false),
    (Module, "interface\nconstant a = 1\n", false),
    (Module, "system\nconstant a = 1\n", false),
    (Module, "interface I\n", false),
    (Module, "passive component C {\n event\n}\n", false),
    (Module, "passive component C {\n internal port\n}\n", false),
    (
        Module,
        "passive component C {\n product container\n}\n",
        false,
    ),
    (Module, "passive component C {\n product record\n}\n", false),
    (
        Module,
        "passive component C {\n state machine instance\n}\n",
        false,
    ),
    (
        Module,
        "passive component C {\n sync input port\n}\n",
        false,
    ),
    (
        Module,
        "passive component C {\n command recv port\n}\n",
        false,
    ),
    (
        Module,
        "module M { topology T {\n connections\n} }\n",
        false,
    ),
    (
        Module,
        "module M { topology T {\n telemetry packets\n} }\n",
        false,
    ),
    (
        Module,
        "module M { topology T { telemetry packets P {\n packet\n} } }\n",
        false,
    ),
    (
        Module,
        "state machine S {\n signal\n action\n guard\n choice\n state\n}\n",
        false,
    ),
    (Module, "module template T(constant : U32) { }\n", false),
    (Module, "module template T(type) { }\n", false),
    (Module, "module template T(instance : I) { }\n", false),
    // A keyword that begins a definition only together with the next one
    (Module, "deployment\n", false),
    (Module, "deployment x\n", false),
    (Module, "module M { deployment }\n", false),
    (Module, "deployment component C { }\n", false),
    (Module, "deployment topology T { }\n", true),
    (Module, "passive component C { external }\n", false),
    (
        Module,
        "passive component C { external param p: U32 }\n",
        true,
    ),
    (Module, "type T = U32 << 2\n", false),
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
