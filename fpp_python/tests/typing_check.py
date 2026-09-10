"""Typed usage of the native bindings, checked by mypy (not run as a test).

`.github/workflows/python.yml` builds the extension and runs mypy over this file
so the checked-in `fpp_python/fpp.pyi` stub is validated against real call
sites. It exercises both entry points — `parse` (a `SyntaxTree` of `TransUnit`s)
and `analyze` (a `Model`) — and the semantic surface: the `Analysis` root, its typed maps
(`dict[Symbol, Component]`, `dict[int, Command]`), the `Type` / `Command` /
`NonParamKind` union base+subclass hierarchies (narrowed with `isinstance`), the
resolved `Loc` and lazy `Span` location types, the state-machine model, and the AST
traversal surface (`AstNode.children` plus a `NodeVisitor` subclass overriding typed
`visit_*` methods).
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Optional

from fpp import (
    analyze,
    parse,
    Analysis,
    Async,
    AstNode,
    Command,
    Component,
    DefComponent,
    DefConstant,
    Endpoint,
    IntegerKind,
    Loc,
    Model,
    NodeVisitor,
    NonParam,
    NonParamKind,
    PrimitiveInt,
    Span,
    SpecCommand,
    StateMachine,
    StateMachineSymbol,
    Symbol,
    SyntaxTree,
    TransUnit,
    Type,
)

SRC = """
module Fw {
  port Cmd
  port CmdReg
  port CmdResponse
}
module M {
  constant c = 42
  port P
  active component A {
    command recv port cmdIn
    command reg port cmdRegOut
    command resp port cmdResponseOut
    async command DO_IT(arg: U32)
    sync input port pIn: P
    output port pOut: P
  }
  instance a1: A base id 0x100 queue size 10
  instance a2: A base id 0x200 queue size 10
  topology T {
    instance a1
    instance a2
    connections C { a1.pOut -> a2.pIn }
  }
  state machine SM {
    action a
    signal s
    initial enter S1
    state S1 { on s enter S1 }
  }
}
"""


def command_priority(cmd: Command) -> Optional[int]:
    """`Command` is itself a union (`NonParam | CommandParam`); narrowing to
    `NonParam` gives a `.kind` of the `NonParamKind` union (`Async | Guarded |
    Sync`), narrowed again to the `Async` subclass, which alone exposes
    `.priority`."""
    if not isinstance(cmd, NonParam):
        return None
    kind: NonParamKind = cmd.kind
    if isinstance(kind, Async):
        return kind.priority  # Optional[int], only on the Async subclass
    return None


def constant_type_kind(node: DefConstant) -> Optional[IntegerKind]:
    """`.resolved_type` is the `Type` union (`Optional`); `isinstance` narrows it
    to `PrimitiveInt`, whose `.value` is the mirrored `IntegerKind` payload."""
    resolved: Optional[Type] = node.resolved_type
    if isinstance(resolved, PrimitiveInt):
        return resolved.value
    return None


def analysis_detail(a: Analysis) -> int:
    """Navigate the typed semantic mirror: the component map is
    `dict[Symbol, Component]`; each component's `command_map` is
    `dict[int, Command]` keyed by opcode, and its definition node carries an
    `Optional[Loc]`."""
    total = 0
    for sym, comp in a.component_map.items():
        symbol: Symbol = sym
        component: Component = comp
        total += len(a.get_qualified_name(symbol))
        loc: Optional[Loc] = component.node.location
        if loc is not None:
            line: int = loc.line
            uri: str = loc.uri
            total += line + len(uri)
        commands: dict[int, Command] = component.command_map
        for opcode, cmd in commands.items():
            total += opcode + len(cmd.name)
            prio = command_priority(cmd)
            total += prio if prio is not None else 0
    for sm_sym, sm in a.state_machine_map.items():
        machine: StateMachine = sm
        actions: list[StateMachineSymbol] = machine.actions
        total += len(actions)
    return total


def connection_spans(a: Analysis) -> int:
    """`Endpoint.loc` is a lazy `Span`: the file/line resolve on demand, and
    `resolve()` yields the concrete `Loc`."""
    total = 0
    for topology in a.topology_map.values():
        for connections in topology.connection_map.values():
            for conn in connections:
                source: Endpoint = conn.from_
                span: Span = source.loc
                resolved: Loc = span.resolve()
                total += span.line + resolved.column + len(span.uri)
    return total


class CommandCollector(NodeVisitor):
    """A typed traversal: each overridden `visit_*` receives its concrete node
    class, and `super().visit_<TypeName>(node)` continues into the children."""

    def __init__(self) -> None:
        self.components: list[str] = []
        self.commands: list[str] = []

    def visit_DefComponent(self, node: DefComponent) -> Any:
        self.components.append(node.name)
        return super().visit_DefComponent(node)

    def visit_SpecCommand(self, node: SpecCommand) -> Any:
        self.commands.append(node.name)
        return super().visit_SpecCommand(node)

    def generic_visit(self, node: AstNode) -> Any:
        kids: list[AstNode] = node.children
        for kid in kids:
            assert kid.node_id >= 0
        return super().generic_visit(node)


def descendant_count(node: AstNode) -> int:
    """`children` is the type-agnostic traversal primitive: `list[AstNode]`."""
    total = 1
    for child in node.children:
        total += descendant_count(child)
    return total


def syntax_only(paths: list[str]) -> int:
    """`parse` yields a `SyntaxTree` of `TransUnit`s and no `Analysis`."""
    tree: SyntaxTree = parse(paths)
    if tree.has_errors:
        for diag in tree.diagnostics:
            print(diag.level, diag.message)
        return tree.error_count

    total = 0
    for unit in tree.units:
        uri: str = unit.uri
        members: list[AstNode] = unit.members
        total += len(uri) + len(members) + len(unit)
    return total


def main() -> int:
    model: Model = analyze(source=SRC, uri="mem.fpp")
    if model.has_errors:
        for diag in model.diagnostics:
            print(diag.level, diag.message)
        return model.error_count

    node_id_sum = syntax_only([str(Path(__file__).parent / "commands" / "commands.fpp")])
    visitor = CommandCollector()
    units: list[TransUnit] = model.ast
    for node in (n for unit in units for n in unit.members):
        node_id_sum += node.node_id + descendant_count(node)
        if isinstance(node, DefConstant):
            constant_type_kind(node)
        visitor.visit(node)
    node_id_sum += len(visitor.components) + len(visitor.commands)

    analysis: Analysis = model.analysis
    sym: Optional[Symbol] = model.lookup("M.c")
    name = analysis.get_qualified_name(sym) if sym is not None else "<none>"
    total = node_id_sum + analysis_detail(analysis) + connection_spans(analysis)
    print(name, total)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
