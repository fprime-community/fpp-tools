"""The API contracts that were previously either wrong or undefined.

One module per promise rather than per class, because each of these is a promise
a *consumer* relies on — an autocoder walking the model — and each was reported
from that side: an ambiguous `lookup`, an enum whose member name was unreachable,
a map that iterated in hash order, a string that lost its declared size, a visitor
whose documented example did not run, a `Topology` missing the `.node` every
sibling has, a symbol `.node` that was an int, a union subclass whose name did not
say which union it belonged to, a type that could not render itself, and a
diagnostic off by one.
"""

import enum

import pytest

import fpp as f

# `string` (unsized) and `serialized_size` need the framework definitions, so the
# fixtures that use a bare string carry them.
FRAMEWORK = """
type FwSizeStoreType = U16
constant FW_FIXED_LENGTH_STRING_SIZE = 256
"""

# `type Time` and `port Time` side by side: one qualified name, two symbols. This
# is F Prime's own `Fw/Time/Time.fpp`, reduced.
COLLISION_SRC = """
module Fw {
  type Time
  port Time
}
"""

STATE_MACHINE_SRC = """
module SM {
  state machine A {
    signal Sig
    initial enter S
    state S { on Sig enter T }
    state T { on Sig enter S }
  }
}
"""

# One member per string context that accepts `string size N`, plus enough of a
# component to be legal.
STRING_SIZES_SRC = (
    FRAMEWORK
    + """
module Ref {
  struct S { y: string size 40 }
  port P(msg: string size 30)
  active component C {
    command reg port cmdRegOut
    command recv port cmdIn
    command resp port cmdResp
    sync input port p: P
    async command CMD(b: string size 20)
    param NAME: string size 16
    telemetry Label: string size 12
    event port eventOut
    text event port textEventOut
    time get port timeGetOut
    telemetry port tlmOut
    param get port prmGetOut
    param set port prmSetOut
  }
}
module Fw {
  port Cmd
  port CmdReg
  port CmdResponse
  port Log
  port LogText
  port Time
  port Tlm
  port PrmGet
  port PrmSet
}
"""
)

# A struct whose members are declared in an order no hash agrees with, and a
# component with enough commands that opcode order is visibly not hash order.
ORDER_SRC = """
module M {
  struct Complex {
    x: U32
    y: U8
    u: F32
    w: bool
    z: U8
    b: I16
    d: F64
    i: U16
    q: I8
    a: U64
    s: I64
    v: I32
  }
  passive component Many {
    command reg port cmdRegOut
    command recv port cmdIn
    command resp port cmdResp
    sync command C0
    sync command C1
    sync command C2
    sync command C3
    sync command C4
    sync command C5
    sync command C6
    sync command C7
    sync command C8
    sync command C9
  }
  passive component Zeta {}
  passive component Alpha {}
  passive component Mid {}
}
module Fw {
  port Cmd
  port CmdReg
  port CmdResponse
}
"""

# The declaration order of `ORDER_SRC`'s components, which is deliberately neither
# alphabetical nor reverse-alphabetical, so a symbol-keyed map that came out in
# name order or in hash order would not match it.
COMPONENTS_IN_DECLARATION_ORDER = ["Many", "Zeta", "Alpha", "Mid"]


def analyzed(src: str) -> f.Model:
    m = f.analyze(source=src)
    assert not m.has_errors, [d.display for d in m.diagnostics]
    return m


# --- lookup: a name can denote more than one symbol -------------------------


def test_lookup_all_returns_every_symbol_of_a_name():
    m = analyzed(COLLISION_SRC)
    syms = m.lookup_all("Fw.Time")
    assert [type(s).__name__ for s in syms] == ["SymbolAbsType", "SymbolPort"]


def test_lookup_returns_the_first_declared_not_an_arbitrary_one():
    m = analyzed(COLLISION_SRC)
    # `type Time` is declared first, so it wins — deterministically, where the
    # winner used to be whichever the symbol map happened to reach last.
    assert type(m.lookup("Fw.Time")).__name__ == "SymbolAbsType"
    assert m.lookup("Fw.Time").node_id == min(s.node_id for s in m.lookup_all("Fw.Time"))


def test_lookup_kind_selects_the_name_group():
    m = analyzed(COLLISION_SRC)
    abs_type = m.lookup("Fw.Time", kind=f.SymbolAbsType)
    port = m.lookup("Fw.Time", kind=f.SymbolPort)
    assert type(abs_type).__name__ == "SymbolAbsType"
    assert type(port).__name__ == "SymbolPort"
    # The failure this prevents: reaching for the type and getting the port,
    # whose definition has no resolved type.
    assert abs_type.definition.resolved_type is not None
    assert port.definition.resolved_type is None


def test_lookup_kind_filters_lookup_all_too():
    m = analyzed(COLLISION_SRC)
    assert len(m.lookup_all("Fw.Time", kind=f.SymbolPort)) == 1
    assert m.lookup_all("Fw.Time", kind=f.SymbolComponent) == []
    assert m.lookup("Fw.Time", kind=f.SymbolComponent) is None


def test_lookup_of_an_unknown_name_is_none():
    m = analyzed(COLLISION_SRC)
    assert m.lookup("Fw.Nope") is None
    assert m.lookup_all("Fw.Nope") == []


def test_lookup_kind_must_be_a_symbol_class():
    m = analyzed(COLLISION_SRC)
    with pytest.raises(TypeError):
        m.lookup("Fw.Time", kind=int)
    with pytest.raises(TypeError):
        m.lookup("Fw.Time", kind="SymbolPort")


def test_a_reopened_module_yields_one_symbol_not_several():
    m = analyzed("module M { constant a = 1 }\nmodule M { constant b = 2 }\n")
    assert len(m.lookup_all("M")) == 1


# --- every union subclass is named for its union ---------------------------


# The unions whose subclasses read `<Union><Variant>` instead of the usual
# `<Variant><Union>`: the two symbol unions, spelled the way the compiler spells
# them (`Symbol::Port` -> `SymbolPort`).
PREFIXED_UNIONS = {"Symbol", "StateMachineSymbol"}

# Bare names that a union variant used to claim, before every subclass was named
# for its union. Removed, not aliased: a name that says nothing about which union
# it belongs to (`fpp.Array` beside `fpp.ArrayType`, `fpp.Float` beside
# `fpp.FloatValue`) is the confusion the rule exists to end, so keeping it would
# defeat the change.
REMOVED_BARE_NAMES = [
    "Action",
    "Array",
    "Async",
    "AsyncInput",
    "Constant",
    "EnumConstant",
    "External",
    "Float",
    "Guard",
    "Guarded",
    "GuardedInput",
    "Initial",
    "InitialTransition",
    "Internal",
    "Literal",
    "Module",
    "NonParam",
    "Output",
    "Port",
    "PrimitiveInt",
    "Rational",
    "Serial",
    "Signal",
    "StateEntry",
    "StateExit",
    "StateTransition",
    "Struct",
    "Sync",
    "SyncInput",
    "System",
]


@pytest.mark.parametrize("name", REMOVED_BARE_NAMES)
def test_the_bare_variant_names_are_gone(name):
    assert not hasattr(f, name)
    assert name not in f.__all__


def _union_subclasses(base: type) -> set[str]:
    return {
        name
        for name in dir(f)
        if isinstance(getattr(f, name), type)
        and issubclass(getattr(f, name), base)
        and getattr(f, name) is not base
    }


def test_every_symbol_subclass_carries_the_symbol_prefix():
    # `Symbol` is the union alias and `SymbolBase` the base class; the rest are
    # the fifteen concrete subclasses.
    subclasses = _union_subclasses(f.SymbolBase)
    assert len(subclasses) == 15
    assert all(name.startswith("Symbol") for name in subclasses), sorted(subclasses)


def test_every_union_subclass_is_named_for_its_union():
    # Each union contributes a `<Union>Base` class, so the bases enumerate the
    # unions without a hand-maintained list — a union added upstream is covered
    # the day it appears.
    bases = {
        name[: -len("Base")]: getattr(f, name)
        for name in dir(f)
        if name.endswith("Base") and isinstance(getattr(f, name), type)
    }
    assert "Symbol" in bases and "Type" in bases and "Value" in bases
    for union, base in bases.items():
        subclasses = _union_subclasses(base)
        assert subclasses, union
        for name in subclasses:
            if union in PREFIXED_UNIONS:
                assert name.startswith(union), (union, name)
            else:
                assert name.endswith(union), (union, name)


# --- a symbol's `.node` is an id, so it is spelled `node_id` ---------------


def test_symbol_node_id_is_the_id_and_definition_is_the_node():
    m = analyzed("module M { constant a = 1 }")
    sym = m.lookup("M.a")
    assert isinstance(sym.node_id, int)
    assert sym.definition.node_id == sym.node_id
    assert type(sym.definition).__name__ == "DefConstant"


def test_the_int_valued_node_attribute_is_gone():
    # Removed rather than aliased: `.node` returning an `int` while every other
    # `.node` in the API returns an object is the trap the rename removes.
    m = analyzed("module M { constant a = 1 }")
    assert not hasattr(m.lookup("M.a"), "node")
    assert not hasattr(f.SymbolBase, "node")
    assert not hasattr(f.UseDefMatching, "node")


# --- enums expose their member name ---------------------------------------


def test_enum_members_expose_name_and_value():
    assert f.IntegerKind.U32.name == "U32"
    assert f.IntegerKind.U32.value == "U32"
    assert f.Direction.Output.name == "Output"
    assert f.EventSeverity.ActivityHigh.name == "ActivityHigh"


def test_enum_member_name_matches_its_repr():
    # The member name used to be recoverable only by parsing this.
    for member in (f.IntegerKind.I8, f.FloatKind.F64, f.ComponentKind.Passive):
        cls, _, name = repr(member).partition(".")
        assert name == member.name
        assert cls == type(member).__name__


def test_enums_are_not_enum_enum_subclasses():
    # Documented, and now what the stub says too — so a type checker rejects the
    # `enum.Enum` class surface instead of letting it fail at runtime.
    assert not isinstance(f.IntegerKind.U32, enum.Enum)
    assert not issubclass(f.IntegerKind, enum.Enum)
    with pytest.raises(TypeError):
        list(f.IntegerKind)


def test_an_enum_valued_field_reads_back_as_a_member():
    m = analyzed("module M { array A = [4] U32 }")
    elt = m.analysis.type_map[m.lookup("M.A").node_id].anon_array.elt_type
    assert elt.value is f.IntegerKind.U32
    assert elt.value.name == "U32"


# --- maps iterate in a defined order --------------------------------------


def test_id_keyed_maps_iterate_in_key_order():
    m = analyzed(ORDER_SRC)
    component = m.analysis.component_map[m.lookup("M.Many")]
    opcodes = list(component.command_map)
    assert opcodes == sorted(opcodes)
    assert len(opcodes) == 10


def test_name_keyed_maps_iterate_in_name_order():
    m = analyzed(ORDER_SRC)
    component = m.analysis.component_map[m.lookup("M.Many")]
    names = list(component.port_map)
    assert names == sorted(names)


def test_node_keyed_maps_iterate_in_definition_order():
    m = analyzed(ORDER_SRC)
    a = m.analysis
    assert list(a.type_map) == sorted(a.type_map)
    assert list(a.symbol_map) == sorted(a.symbol_map)
    assert list(a.use_def_map) == sorted(a.use_def_map)


def test_symbol_keyed_maps_iterate_in_declaration_order():
    m = analyzed(ORDER_SRC)
    # Four components, declared in an order that is neither alphabetical nor its
    # reverse — so this pins declaration order rather than passing by luck.
    names = [s.unqualified_name for s in m.analysis.component_map]
    assert names == COMPONENTS_IN_DECLARATION_ORDER
    ids = [s.node_id for s in m.analysis.component_map]
    assert ids == sorted(ids)


def test_a_union_keyed_map_orders_by_its_key_s_node():
    # `type_option_map` is keyed by a `StateMachineTypedElement`, which is neither a
    # symbol nor an ordered key — but it does expose a node id, which is what the
    # ordering rule keys on.
    m = analyzed(STATE_MACHINE_SRC)
    (machine,) = m.analysis.state_machine_map.values()
    ids = [element.node_id for element in machine.sma.type_option_map]
    assert ids
    assert ids == sorted(ids)


def test_anon_struct_members_are_name_ordered_and_the_node_is_declaration_ordered():
    m = analyzed(ORDER_SRC)
    struct = m.analysis.type_map[m.lookup("M.Complex").node_id]
    members = list(struct.anon_struct.members)
    assert members == sorted(members)
    # The analysis stores these unordered, so declaration order lives on the AST
    # node — which is what the docs now point at.
    declared = [member.name for member in struct.node.members]
    assert declared != members
    assert sorted(declared) == members


# --- a modeled string keeps its declared size everywhere ------------------


def test_string_size_is_recorded_in_every_context_that_accepts_one():
    m = analyzed(STRING_SIZES_SRC)

    class Strings(f.NodeVisitor):
        def __init__(self):
            self.sizes = []

        def visit_TypeName(self, node):
            if type(node.kind).__name__ == "TypeNameString":
                self.sizes.append(node.resolved_type.value)
            super().visit_TypeName(node)

    walk = Strings()
    walk.visit(m)
    # struct member, port param, command param, component param, tlm channel
    assert sorted(walk.sizes) == [12, 16, 20, 30, 40]


def test_a_sized_string_outside_a_type_definition_reaches_the_semantic_type():
    m = analyzed(STRING_SIZES_SRC)
    component = m.analysis.component_map[m.lookup("Ref.C")]
    (param,) = component.param_map.values()
    assert param.param_type.value == 16
    (channel,) = component.tlm_channel_map.values()
    assert channel.channel_type.value == 12


def test_an_invalid_string_size_is_reported_outside_a_type_definition_too():
    bad = f.analyze(source=FRAMEWORK + "module M { port P(m: string size -1) }")
    assert bad.has_errors
    assert "negative string sizes" in bad.diagnostics[0].message


# --- sources vs imports --------------------------------------------------


def _split_model(tmp_path):
    lib = tmp_path / "lib.fpp"
    app = tmp_path / "app.fpp"
    lib.write_text("module Shared { constant WIDTH = 8 }\n")
    app.write_text("module App { constant total = Shared.WIDTH * 4 }\n")
    return f.analyze([str(app)], imports=[str(lib)]), app, lib


def test_imports_are_analyzed_but_not_sources(tmp_path):
    m, app, lib = _split_model(tmp_path)
    assert not m.has_errors, [d.display for d in m.diagnostics]
    assert [(u.uri, u.is_source) for u in m.ast] == [(str(app), True), (str(lib), False)]
    # The import resolved a reference — which is the point of passing it.
    assert m.lookup("App.total").definition.value.resolved_value.value == 32


def test_in_source_follows_the_unit_not_the_file(tmp_path):
    m, _, _ = _split_model(tmp_path)
    (source_unit,) = [u for u in m.ast if u.is_source]
    (import_unit,) = [u for u in m.ast if not u.is_source]
    assert source_unit.members[0].in_source is True
    assert import_unit.members[0].in_source is False


def test_an_included_member_is_in_source_though_its_file_was_not_passed():
    m = f.analyze("tests/includes/host.fpp")
    (host,) = m.ast[0].members
    (included,) = host.members
    assert included.location.uri.endswith("inc.fppi")
    assert included.in_source is True


def test_imports_alone_is_nothing_to_compile(tmp_path):
    lib = tmp_path / "lib.fpp"
    lib.write_text("module Shared { constant WIDTH = 8 }\n")
    with pytest.raises(ValueError):
        f.analyze(imports=[str(lib)])


def test_everything_parse_returns_is_a_source():
    tree = f.parse(source="module M { constant a = 1 }")
    assert all(u.is_source for u in tree.units)


# --- the visitor takes a whole model ------------------------------------


class Components(f.NodeVisitor):
    def __init__(self):
        self.names = []

    def visit_DefComponent(self, node):
        self.names.append(node.name)
        super().visit_DefComponent(node)


def test_visit_accepts_a_model_a_unit_and_a_node():
    m = analyzed(ORDER_SRC)
    whole = Components()
    whole.visit(m)
    explicit = Components()
    for unit in m.ast:
        for member in unit.members:
            explicit.visit(member)
    per_unit = Components()
    for unit in m.ast:
        per_unit.visit(unit)
    # Same components, and in source order, whichever entry point started the walk.
    assert whole.names == explicit.names == per_unit.names
    assert whole.names == COMPONENTS_IN_DECLARATION_ORDER


def test_visit_accepts_a_syntax_tree():
    tree = f.parse(source=ORDER_SRC)
    v = Components()
    v.visit(tree)
    assert v.names == COMPONENTS_IN_DECLARATION_ORDER


def test_visit_rejects_anything_else():
    with pytest.raises(TypeError):
        f.NodeVisitor().visit(42)
    with pytest.raises(TypeError):
        f.NodeVisitor().visit("module M {}")


def test_the_container_entry_points_are_overridable():
    m = analyzed(ORDER_SRC)

    class Counting(f.NodeVisitor):
        def __init__(self):
            self.units = 0

        def visit_TransUnit(self, unit):
            self.units += 1
            super().visit_TransUnit(unit)

    v = Counting()
    v.visit(m)
    assert v.units == len(m.ast) == 1


def test_generic_visit_still_takes_ast_nodes_only():
    # A container is not a node, so it does not come through the funnel that
    # claims to see every node — which is what keeps an existing
    # `generic_visit(self, node: AstNode)` override valid.
    m = analyzed(ORDER_SRC)

    class OnlyNodes(f.NodeVisitor):
        def __init__(self):
            self.seen = []

        def generic_visit(self, node):
            self.seen.append(node)
            return super().generic_visit(node)

    v = OnlyNodes()
    v.visit(m)
    assert v.seen
    assert all(isinstance(n, f.AstNode) for n in v.seen)


# --- Topology.node -------------------------------------------------------


def test_topology_exposes_its_definition_node_like_every_sibling():
    m = analyzed(
        """
module M {
  port P
  passive component C1 { output port pOut: P }
  passive component C2 { sync input port pIn: P }
  instance c1: C1 base id 0x100
  instance c2: C2 base id 0x200
  topology T {
    instance c1
    instance c2
    connections C { c1.pOut -> c2.pIn }
  }
}
"""
    )
    (symbol, topology) = next(iter(m.analysis.topology_map.items()))
    assert isinstance(topology.node, f.DefTopology)
    assert topology.node.name == "T"
    # A recorded walk node, not one built fresh: identity with the symbol's.
    assert topology.node is symbol.definition


# --- diagnostics ---------------------------------------------------------


def test_diagnostic_display_is_one_indexed_and_location_is_zero_indexed():
    bad = f.analyze(source="module Bad { struct S { x: Nope } }")
    (diagnostic, *_) = bad.diagnostics
    assert diagnostic.location.line == 0
    assert diagnostic.location.column == 27
    assert diagnostic.display == (
        "<string>:1:28: error: cannot find type `Nope` in scope"
    )
    assert str(diagnostic) == diagnostic.display


def test_loc_and_span_render_the_same_position_as_a_diagnostic():
    bad = f.analyze(source="module Bad { struct S { x: Nope } }")
    location = bad.diagnostics[0].location
    assert location.display == "<string>:1:28"
    assert str(location) == location.display
    assert repr(location) == f"Loc({location.display})"


# --- str is the compiler's rendering, repr names the class -----------------


def test_str_is_the_compilers_own_rendering_of_a_type():
    # `str` is the native `Display` — the same text the compiler puts in a
    # diagnostic — so it is the model's spelling, not a re-derived one.
    m = analyzed(
        FRAMEWORK
        + """
module Ref {
  type Abs
  array Arr = [3] U32
  enum E { A, B }
  struct Simple { x: U32, y: string size 40, b: bool, f: F32 }
}
"""
    )
    a = m.analysis

    def spelled(name):
        return str(a.type_map[m.lookup(name).node_id])

    assert spelled("Ref.Abs") == "Abs"
    assert spelled("Ref.Arr") == "Arr"
    assert spelled("Ref.E") == "E"
    assert spelled("Ref.Simple") == "Simple"

    simple = a.type_map[m.lookup("Ref.Simple").node_id]
    members = simple.anon_struct.members
    assert str(members["x"]) == "U32"
    assert str(members["f"]) == "F32"
    # A declared size is not part of how the compiler renders a string type.
    assert str(members["y"]) == "string"
    # Every member renders by `Display` too, so an aggregate stays one line.
    assert (
        str(simple.anon_struct)
        == "anonymous struct { b: boolean, f: F32, x: U32, y: string }"
    )

    arr = a.type_map[m.lookup("Ref.Arr").node_id]
    assert str(arr.anon_array) == "[3] U32"


def test_str_falls_back_to_repr_without_a_native_display():
    m = analyzed(ORDER_SRC)
    component = m.analysis.component_map[m.lookup("M.Many")]
    # `Component` has no native `Display`; Python then falls back to `__repr__`
    # rather than the bindings inventing a second rendering.
    assert str(component) == repr(component)


def test_repr_names_the_concrete_class_and_identifies_the_value():
    m = analyzed("module M { array A = [3] U32\n constant c = 6 * 7 }")
    a = m.analysis

    # A type: the class, then the compiler's rendering of the value.
    array = a.type_map[m.lookup("M.A").node_id]
    assert repr(array) == "<ArrayType A>"
    assert repr(array.anon_array.elt_type) == "<PrimitiveIntType U32>"

    # A value: likewise, so a folded constant reads as what it folded to.
    folded = m.lookup("M.c").definition.value.resolved_value
    assert repr(folded) == "<IntegerValue 42>"

    # A symbol has no rendering of its own: its qualified name identifies it, and
    # the class says which kind of definition it points at.
    assert repr(m.lookup("M.A")) == "<SymbolArrayType 'M.A'>"

    # An entity names itself the way the model names it: a component through its
    # symbol, a sub-element through its own member name.
    order = analyzed(ORDER_SRC)
    component = order.analysis.component_map[order.lookup("M.Many")]
    assert repr(component) == "<Component 'M.Many'>"
    first_by_opcode = next(iter(component.command_map.values()))
    assert repr(first_by_opcode) == "<NonParamCommand 'C0'>"
