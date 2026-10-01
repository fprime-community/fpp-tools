"""Typed usage of the native bindings, checked by mypy (not run as a test).

`.github/workflows/python.yml` builds the extension and runs mypy over this file
so the checked-in stubs (`fpp_python/python/fpp/__init__.pyi` and `ast.pyi`) are
validated against real call sites. It exercises both entry points — `parse` (a `SyntaxTree` of `TransUnit`s)
and `analyze` (a `Model`) — and the semantic surface: the `Analysis` root, its typed maps
(`dict[Symbol.Variant, Component]`, `dict[int, Command.Variant]`), the `Type` /
`Command` / `NonParamKind` unions — a base class carrying nested variant classes,
narrowed with `isinstance` and matched exhaustively against `<Base>.Variant` — the
resolved `Loc` and lazy `Span` location types, the state-machine model, and the AST
traversal surface (`AstNode.children` plus a `AstVisitor` subclass overriding typed
`visit_*` methods). The AST half comes from the `fpp.ast` submodule, so the two
stub files are checked against each other as well: `PortInstanceIdentifier` is
imported from `fpp` and is the semantic entity, while `fpp.ast` has a node of that
name. It also covers the contracts the stub is easiest to get wrong
about: a leaf enum's `str`-typed `.name`/`.value`, `str()` of a union base,
`lookup(kind=…)` / `lookup_all`, a symbol's `int` `node_id` beside its node
`definition`, `Topology.node`, `Diagnostic.display`, the writable `Diagnostic` raised as a
`DiagnosticError`, and the `is_source` / `in_source` flags.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Optional, assert_never, assert_type

from fpp import (
    analyze,
    parse,
    Analysis,
    Command,
    Component,
    Diagnostic,
    DiagnosticMessageKind,
    DiagnosticLevel,
    DiagnosticMessage,
    DiagnosticError,
    Endpoint,
    InterfaceInstance,
    Loc,
    Model,
    AstVisitor,
    NonParamKind,
    PortInstanceIdentifier,
    Span,
    StateMachine,
    StateMachineSymbol,
    Symbol,
    SyntaxTree,
    Topology,
    TransUnit,
    Type,
    Value,
)
from fpp.ast import (
    AstNode,
    Ident,
    DefComponent,
    DefConstant,
    DefPort,
    DefTopology,
    Expr,
    ExprKind,
    IntegerKind,
    SpecCommand,
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


def command_priority(cmd: Command.Variant) -> Optional[int]:
    """`Command.Variant` is the closed union (`Command.NonParam | Command.Param`);
    narrowing to `Command.NonParam` gives a `.kind` of `NonParamKind.Variant`,
    narrowed again to the async variant, which alone exposes `.priority`."""
    if not isinstance(cmd, Command.NonParam):
        return None
    kind: NonParamKind.Variant = cmd.kind
    if isinstance(kind, NonParamKind.Async):
        return kind.priority  # Optional[int], only on the async variant
    return None


def constant_type_kind(node: DefConstant) -> Optional[IntegerKind]:
    """`.resolved_type` is `Type.Variant` (`Optional`); `isinstance` narrows it to
    `Type.PrimitiveInt`, whose `.value` is the mirrored `IntegerKind` payload."""
    resolved: Optional[Type.Variant] = node.resolved_type
    if isinstance(resolved, Type.PrimitiveInt):
        return resolved.value
    return None


def describe_value(v: Value.Variant) -> str:
    """Every `Value` variant, matched exhaustively.

    This is what notices a variant added to `fpp_analysis::semantics::Value`:
    `Value.Variant` is a genuinely closed union, so `assert_never` type-checks only
    while every arm is present, and mypy names the one that is missing. Annotating
    the base class `Value` instead would silently accept an incomplete match — a base
    class is open, so nothing narrows it to `Never`.
    """
    match v:
        case Value.PrimitiveInteger():
            return f"primitive {v.value} {v.kind.name}"
        case Value.AbsType():
            return f"abstract {v.ty.node.name}"
        case Value.Integer():
            return f"integer {v.value}"
        case Value.Float():
            return f"float {v.value} {v.kind.name}"
        case Value.Boolean():
            return f"bool {v.value}"
        case Value.String():
            return f"string {v.value}"
        case Value.EnumConstant():
            return f"enum constant {v.value[0]}"
        case Value.AnonArray():
            return f"anon array {len(v.elements)}"
        case Value.Array():
            return f"array {v.ty.node.name}"
        case Value.AnonStruct():
            return f"anon struct {len(v.members)}"
        case Value.Struct():
            return f"struct {v.ty.node.name}"
        case _:
            assert_never(v)


def describe_type(t: Type.Variant) -> str:
    """Every `Type` variant, narrowed through its nested classes.

    Unlike `describe_value` this is NOT an exhaustiveness check, and cannot be:
    `Type` is the one union whose bare base is a member of its own closed union — an
    unknown type whose definition node is absent from the walk surfaces as the base —
    so the `Type()` arm it needs is a class pattern that also matches every variant.
    Drop an arm above it and that arm absorbs the remainder, leaving `Never` and no
    complaint. `describe_value` is the canary; this exercises the narrowing.

    The `Type()` arm must come last for the same reason: placed earlier it would
    swallow everything below it.
    """
    match t:
        case Type.PrimitiveInt():
            return f"primitive {t.value.name}"
        case Type.Float():
            return f"float {t.value.name}"
        case Type.String():
            return f"string {t.value}"
        case Type.Boolean():
            return "bool"
        case Type.Integer():
            return "integer"
        case Type.Abs():
            return f"abs {t.node.name}"
        case Type.Alias():
            return f"alias {describe_type(t.alias_type)}"
        case Type.Array():
            return f"array {t.node.name}"
        case Type.AnonArray():
            return f"anon array {t.size}"
        case Type.Enum():
            return f"enum {t.node.name}"
        case Type.Struct():
            return f"struct {t.node.name}"
        case Type.AnonStruct():
            return f"anon struct {len(t.members)}"
        case Type():
            return "unknown"
        case _:
            assert_never(t)


def describe_expr(e: Expr) -> str:
    """Every `ExprKind` variant, matched exhaustively.

    The AST kind enums nest exactly as the semantic unions do, and none of them has
    the bare base as a member, so unlike `describe_type` this really is an
    exhaustiveness check. Before they had a base class at all, `ExprBinop` and
    `ExprLiteralInt` were unrelated classes and `isinstance(k, ExprKind)` could not be
    asked.
    """
    k: ExprKind.Variant = e.kind
    match k:
        case ExprKind.Array():
            return f"array {len(k.elements)}"
        case ExprKind.ArraySubscript():
            return f"{describe_expr(k.e1)}[{describe_expr(k.e2)}]"
        case ExprKind.Binop():
            return f"({describe_expr(k.left)} {k.op.name} {describe_expr(k.right)})"
        case ExprKind.Dot():
            return f"{describe_expr(k.e)}.{k.id.data}"
        case ExprKind.Ident():
            return k.value
        case ExprKind.LiteralBool():
            return str(k.value)
        case ExprKind.LiteralInt() | ExprKind.LiteralFloat() | ExprKind.LiteralString():
            return k.value
        case ExprKind.Paren():
            return f"({describe_expr(k.value)})"
        case ExprKind.SizeOf():
            return "sizeof"
        case ExprKind.Struct():
            return f"struct {len(k.elements)}"
        case ExprKind.Unop():
            return f"{k.op.name}{describe_expr(k.e)}"
        case _:
            assert_never(k)


def topology_of(instance: InterfaceInstance.Variant) -> Optional[Topology]:
    """A base-class member annotated with a name one of its own variants shadows.

    `as_topology` returns the `Topology` *entity*, while `InterfaceInstance.Topology`
    is a variant of this very union — so inside the class body the bare name is
    ambiguous, and mypy and pyright disagree about which one it means. `stub_gen`
    qualifies the annotation as `fpp.Topology` to keep it pointing at the entity;
    `assert_type` is what would catch that regressing, since the wrong reading still
    type-checks on its own. `expr_dot_id` is the same check on the AST side.
    """
    assert_type(instance.as_topology, Optional[Topology])
    return instance.as_topology


def expr_dot_id(e: ExprKind.Dot) -> Ident:
    """The AST-side peer of `topology_of`, inside `fpp.ast`.

    `ExprKind.Dot.id` is the `Ident` *node*, but `ExprKind.Ident` is a sibling variant
    binding that name in the enclosing class scope — so `stub_gen` writes the
    annotation as `fpp.ast.Ident`. `ExprKind.Binop.op` is the same case against a leaf
    enum rather than a node.
    """
    assert_type(e.id, Ident)
    return e.id


def kind_spelling(kind: IntegerKind) -> str:
    """A leaf-enum mirror's `.name`/`.value` are both `str` — the member spelling.
    They are declared on a plain class, not on `enum.Enum`, so the class surface a
    real enum would offer (iteration, `IntegerKind("U32")`) is correctly absent."""
    name: str = kind.name
    value: str = kind.value
    return name + value


def type_spelling(node: AstNode) -> str:
    """`__str__` is declared on the union base (it is the native `Display`), so a
    type renders without narrowing to a subclass first."""
    resolved: Optional[Type.Variant] = node.resolved_type
    return str(resolved) if resolved is not None else ""


def first_port_symbol(model: Model) -> Optional[Symbol.Port]:
    """`lookup`'s `kind=` takes a symbol class and narrows nothing by itself, so
    the caller still asserts what it asked for; `lookup_all` returns a list."""
    every: list[Symbol.Variant] = model.lookup_all("M.P")
    found: Optional[Symbol.Variant] = model.lookup("M.P", kind=Symbol.Port)
    if isinstance(found, Symbol.Port) and every:
        node_id: int = found.node_id
        definition: DefPort = found.definition
        assert node_id == definition.node_id
        return found
    return None


def source_units(model: Model) -> int:
    """`is_source`/`in_source` are plain bools on the unit and the node."""
    total = 0
    for unit in model.ast:
        if unit.is_source:
            total += sum(1 for n in unit.members if n.in_source)
    return total


def analysis_detail(a: Analysis) -> int:
    """Navigate the typed semantic mirror: the component map is
    `dict[Symbol, Component]`; each component's `command_map` is
    `dict[int, Command]` keyed by opcode, and its definition node carries an
    `Optional[Loc]`."""
    total = 0
    for sym, comp in a.component_map.items():
        symbol: Symbol.Variant = sym
        component: Component = comp
        total += len(a.get_qualified_name(symbol))
        loc: Optional[Loc] = component.node.location
        if loc is not None:
            line: int = loc.line
            uri: str = loc.uri
            total += line + len(uri)
        commands: dict[int, Command.Variant] = component.command_map
        for opcode, cmd in commands.items():
            total += opcode + len(cmd.name)
            prio = command_priority(cmd)
            total += prio if prio is not None else 0
    for sm_sym, sm in a.state_machine_map.items():
        machine: StateMachine = sm
        actions: list[StateMachineSymbol.Variant] = machine.actions
        total += len(actions)
    return total


def instance_port_lookups(a: Analysis) -> int:
    """`InterfaceInstance.Component` and `InterfaceInstance.Topology` both expose
    `get_port_instance_identifier(str) -> PortInstanceIdentifier`, raising
    `DiagnosticError` (it throws a `SemanticError`) if the name doesn't
    resolve to a port on the instance."""
    total = 0
    for ci in a.component_instance_map.values():
        try:
            pii: PortInstanceIdentifier = ci.get_port_instance_identifier("pOut")
            total += len(pii.qualified_name)
        except DiagnosticError:
            pass
    for top in a.topology_map.values():
        for instance in top.instance_map:
            if isinstance(instance, InterfaceInstance.Topology):
                try:
                    pii = instance.get_port_instance_identifier("pOut")
                    total += len(pii.qualified_name)
                except DiagnosticError:
                    pass
    return total


def connection_spans(a: Analysis) -> int:
    """`Endpoint.loc` is a lazy `Span`: the file/line resolve on demand, and
    `resolve()` yields the concrete `Loc`."""
    total = 0
    for topology in a.topology_map.values():
        definition: DefTopology = topology.node
        total += len(definition.name)
        for connections in topology.connection_map.values():
            for conn in connections:
                source: Endpoint = conn.from_
                span: Span = source.loc
                resolved: Loc = span.resolve()
                total += span.line + resolved.column + len(span.uri)
    return total


class CommandCollector(AstVisitor):
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
            print(diag.level.value, diag.message)
        return tree.error_count

    total = 0
    for unit in tree.units:
        uri: str = unit.uri
        members: list[AstNode] = unit.members
        total += len(uri) + len(members) + len(unit)
    return total


def built_diagnostic(node: AstNode) -> str:
    """A diagnostic built in Python renders like one the compiler emitted."""
    span: Span = node.span
    diagnostic: Diagnostic = Diagnostic(
        "built from Python",
        level=DiagnosticLevel.Warning,
        span=span,
        children=[
            DiagnosticMessage(
                "declared here", span=span, kind=DiagnosticMessageKind.Annotation
            ),
            DiagnosticMessage("a standalone note"),
        ],
    )
    level: DiagnosticLevel = diagnostic.level
    location: Optional[Loc] = diagnostic.location
    includes: list[Loc] = diagnostic.includes
    source: Optional[str] = diagnostic.source
    children: list[DiagnosticMessage] = diagnostic.children
    kinds: list[DiagnosticMessageKind] = [child.kind for child in children]
    return "".join(
        [
            level.value,
            diagnostic.message,
            diagnostic.display,
            diagnostic.render(color=False),
            str(diagnostic),
            "" if location is None else location.display,
            "".join(loc.display for loc in includes),
            source or "",
            "".join(kind.name for kind in kinds),
        ]
    )


def raised_diagnostic(node: AstNode) -> str:
    """A diagnostic raised as a `DiagnosticError`, extended by its handler."""
    diagnostic: Diagnostic = Diagnostic("raised from Python")
    diagnostic.level = DiagnosticLevel.Warning
    diagnostic.message = "raised from Python, and rewritten"
    diagnostic.set_span(node.span)
    diagnostic.add_note("a note", span=node.span)
    diagnostic.add_annotation("an annotation")
    diagnostic.add_child(DiagnosticMessage("a prebuilt child"))
    diagnostic.children = diagnostic.children[:2]
    try:
        raise DiagnosticError(diagnostic)
    except DiagnosticError as error:
        carried: Diagnostic = error.diagnostic
        carried.add_note("caught here")
        return carried.display


def main() -> int:
    model: Model = analyze(source=SRC, uri="mem.fpp")
    if model.has_errors:
        for diag in model.diagnostics:
            print(diag.level.value, diag.message)
        return model.error_count

    node_id_sum = syntax_only([str(Path(__file__).parent / "commands" / "commands.fpp")])
    visitor = CommandCollector()
    units: list[TransUnit] = model.ast
    for node in (n for unit in units for n in unit.members):
        node_id_sum += node.node_id + descendant_count(node)
        if isinstance(node, DefConstant):
            constant_type_kind(node)
            node_id_sum += len(describe_expr(node.value))
        visitor.visit(node)
    node_id_sum += len(visitor.components) + len(visitor.commands)

    analysis: Analysis = model.analysis
    sym: Optional[Symbol.Variant] = model.lookup("M.c")
    name = analysis.get_qualified_name(sym) if sym is not None else "<none>"
    total = node_id_sum + analysis_detail(analysis) + connection_spans(analysis)
    total += instance_port_lookups(analysis)
    total += source_units(model) + len(kind_spelling(IntegerKind.U32))
    total += len(type_spelling(units[0].members[0]))
    for _sym, comp in analysis.component_map.items():
        for param in comp.param_map.values():
            default = param.default
            if default is not None:
                total += len(describe_value(default))
    total += 1 if first_port_symbol(model) is not None else 0
    for diag in model.diagnostics:
        total += len(diag.display) + len(str(diag)) + len(diag.children)
    total += len(built_diagnostic(units[0].members[0]))
    total += len(raised_diagnostic(units[0].members[0]))
    print(name, total)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
