"""Analysis-entity navigation: components, instances, topology, connections.

Everything is reached through the `model.analysis` mirror of
`fpp_analysis::Analysis` — its public maps and query methods — rather than any
curated `Model` accessor.
"""

import pytest

import fpp as f
from fpp import (
    ComponentKind,
    Direction,
    GeneralPortInstance,
    Loc,
    PortInstance,
    Span,
    SymbolComponent,
    SymbolTopology,
    TopologyInterfaceInstance,
)

SRC = """
port P
passive component C {
  sync input port pIn: P
  output port pOut: P
}
instance a: C base id 0x100
instance b: C base id 0x200
topology T {
  instance a
  instance b
  connections C1 { a.pOut -> b.pIn }
}
"""


@pytest.fixture(scope="module")
def m():
    model = f.analyze(source=SRC, uri="entities.fpp")
    assert not model.has_errors, [d.message for d in model.diagnostics]
    return model


def test_component_map(m):
    a = m.analysis
    (sym, comp) = next(iter(a.component_map.items()))
    assert isinstance(sym, SymbolComponent)
    # Symbol keys support value lookup.
    assert a.component_map[sym] is not None
    assert a.get_qualified_name(sym) == "C"
    assert comp.node.name == "C"
    # The component kind lives on the definition node, not on the semantic entity.
    assert comp.node.kind == ComponentKind.Passive
    assert isinstance(comp.node.location, Loc)
    assert comp.node.location.uri == "entities.fpp"


def test_component_ports(m):
    (comp,) = m.analysis.component_map.values()
    ports = comp.port_interface.port_map
    assert set(ports) == {"pIn", "pOut"}
    # Both are general (non-special) ports, so they land on the same union member.
    assert all(isinstance(p, PortInstance) for p in ports.values())
    assert all(isinstance(p, GeneralPortInstance) for p in ports.values())
    dirs = {n: p.direction for n, p in ports.items()}
    assert dirs == {"pIn": Direction.Input, "pOut": Direction.Output}
    # `Component.port_map` mirrors the interface's map (fresh wrappers, so compare
    # by name rather than by identity).
    assert {n: p.unqualified_name for n, p in comp.port_map.items()} == {
        n: p.unqualified_name for n, p in ports.items()
    }


def test_component_instances(m):
    a = m.analysis
    insts = {ci.qualified_name: ci for ci in a.component_instance_map.values()}
    assert set(insts) == {"a", "b"}
    assert insts["a"].base_id == 0x100
    assert insts["b"].base_id == 0x200
    # The instance's component symbol keys back into the component map.
    csym = insts["a"].component_symbol
    assert a.component_map[csym].node.name == "C"


def test_component_instance_get_port_instance_identifier(m):
    a = m.analysis
    insts = {ci.qualified_name: ci for ci in a.component_instance_map.values()}
    inst_a = insts["a"]

    pii = inst_a.get_port_instance_identifier("pOut")
    assert pii.qualified_name == "a.pOut"
    assert pii.interface_instance.qualified_name == "a"
    assert pii.port_instance.unqualified_name == "pOut"

    # A name that isn't a port on the instance's component raises a
    # `DiagnosticError` (it throws a `SemanticError`) rather than returning
    # some placeholder.
    with pytest.raises(f.DiagnosticError) as excinfo:
        inst_a.get_port_instance_identifier("nonexistent")
    assert "nonexistent" in excinfo.value.diagnostic.message


def test_topology_connections(m: f.Model):
    a = m.analysis
    (tsym, top) = next(iter(a.topology_map.items()))
    assert isinstance(tsym, SymbolTopology)
    assert top.name == "T"
    # `connection_map` is keyed by connection-graph name.
    assert set(top.connection_map) == {"C1"}
    (conn,) = top.connection_map["C1"]
    src = conn.from_
    assert src.port.qualified_name == "a.pOut"
    assert conn.to.port.qualified_name == "b.pIn"
    assert conn.is_unmatched is False


PORT_NUMBER_SRC = """
port P
passive component C {
  sync input port pIn: [3] P
  output port pOut: [3] P
}
instance a: C base id 0x100
instance b: C base id 0x200
topology T {
  instance a
  instance b
  connections C1 {
    a.pOut -> b.pIn
    a.pOut -> b.pIn
    a.pOut[2] -> b.pIn[1]
  }
}
"""


@pytest.fixture(scope="module")
def numbered_top():
    model = f.analyze(source=PORT_NUMBER_SRC, uri="numbering.fpp")
    assert not model.has_errors, [d.message for d in model.diagnostics]
    (top,) = model.analysis.topology_map.values()
    return top


def test_endpoint_port_number_is_the_written_index(numbered_top):
    """`Endpoint.port_number` is what the source wrote, not what analysis resolved.

    Only the third connection carries explicit indices, so the first two report
    `None` on both endpoints even though the topology is fully analyzed.
    """
    written = [
        (c.from_.port_number, c.to.port_number) for c in numbered_top.connection_map["C1"]
    ]
    assert written == [(None, None), (None, None), (2, 1)]


def test_resolved_port_numbers(numbered_top):
    """The numbering pass's output is on the topology, keyed by connection.

    Reachable two equivalent ways: `get_port_number`, which picks the from- or
    to-side map from the direction of the port instance handed to it, and the maps
    themselves. The two implicit output connections get distinct numbers; input
    connections share number 0 unless written otherwise.
    """
    resolved = []
    for c in numbered_top.connection_map["C1"]:
        got = (
            numbered_top.get_port_number(c.from_.port.port_instance, c),
            numbered_top.get_port_number(c.to.port.port_instance, c),
        )
        # A connection keys the number-map dicts by value, so a wrapper rebuilt by
        # any other getter finds the same entry.
        assert got == (
            numbered_top.from_port_number_map[c],
            numbered_top.to_port_number_map[c],
        )
        resolved.append(got)
    assert resolved == [(0, 0), (1, 0), (2, 1)]


def test_resolve_numbers_fills_in_the_endpoints(numbered_top):
    """`resolve_numbers` is the ergonomic read: a copy with both numbers written in.

    Because an endpoint compares by port name *and* written index, that copy is a
    different connection from the original — so it is the original, not the
    resolved image, that keys the number maps.
    """
    c = numbered_top.connection_map["C1"][1]
    resolved = numbered_top.resolve_numbers(c)
    assert (resolved.from_.port_number, resolved.to.port_number) == (1, 0)
    assert resolved != c
    assert resolved not in numbered_top.from_port_number_map


def test_connection_is_a_usable_dict_key(numbered_top):
    """Value equality + hashing, so the two duplicate connections stay distinct.

    They differ only in where they are written, which is exactly what keeps them
    apart in the port-number maps above.
    """
    first, second, third = numbered_top.connection_map["C1"]
    # Freshly built wrappers over the same connection, never the same object.
    again = numbered_top.connection_map["C1"][0]
    assert again is not first
    assert again == first and hash(again) == hash(first)
    assert first != second and first != third
    assert len({first, second, third, again}) == 3
    # Endpoints compare by port name + written index, so both duplicates' from-ends
    # are equal while the connections are not.
    assert first.from_ == second.from_


def test_span_resolves(m):
    # `Endpoint.loc` is a lazy `Span`; the node getters hand back resolved `Loc`s.
    (top,) = m.analysis.topology_map.values()
    (conn,) = top.connection_map["C1"]
    span = conn.from_.loc
    assert isinstance(span, Span)
    loc = span.resolve()
    assert loc.uri == span.uri == "entities.fpp"
    # `a.pOut` is on the 12th source line (0-indexed 11).
    assert span.line == loc.line == 11
    # Spans are hashable, handle-equal values.
    assert span == conn.from_.loc and hash(span) == hash(conn.from_.loc)


def test_component_location(m):
    (comp,) = m.analysis.component_map.values()
    loc = comp.node.location
    assert loc.uri == "entities.fpp"
    # `passive component C` is on the 3rd source line (0-indexed 2).
    assert loc.line == 2
    assert loc.column == 0


NESTED_TOPOLOGY_SRC = """
port P
passive component C {
  sync input port pIn: P
  output port pOut: P
}
instance c1: C base id 0x100
instance c2: C base id 0x200
topology Inner {
  instance c1
  port innerPort = c1.pOut
}
topology Outer {
  import Inner
  instance c2
  connections C1 { Inner.innerPort -> c2.pIn }
}
"""


@pytest.fixture(scope="module")
def nested_m():
    model = f.analyze(source=NESTED_TOPOLOGY_SRC, uri="nested.fpp")
    assert not model.has_errors, [d.message for d in model.diagnostics]
    return model


def test_topology_instance_get_port_instance_identifier(nested_m):
    a = nested_m.analysis
    outer = next(top for top in a.topology_map.values() if top.unqualified_name == "Outer")
    (inst,) = (k for k in outer.instance_map if isinstance(k, TopologyInterfaceInstance))
    assert inst.qualified_name == "Inner"

    pii = inst.get_port_instance_identifier("innerPort")
    assert pii.qualified_name == "Inner.innerPort"
    assert pii.interface_instance.qualified_name == "Inner"
    assert pii.port_instance.unqualified_name == "innerPort"

    # A name that isn't a top port of the imported topology raises a
    # `DiagnosticError` — "pIn" is a port on the underlying component, not a
    # top port of Inner itself.
    with pytest.raises(f.DiagnosticError) as excinfo:
        inst.get_port_instance_identifier("pIn")
    assert "pIn" in excinfo.value.diagnostic.message
