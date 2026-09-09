"""Analysis-entity navigation: components, instances, topology, connections.

Everything is reached through the `model.analysis` mirror of
`fpp_analysis::Analysis` — its public maps and query methods — rather than any
curated `Model` accessor.
"""

import pytest

import fpp_python as f
from fpp_python import (
    ComponentKind,
    Direction,
    General,
    PortInstance,
    Span,
    SymbolComponent,
    SymbolTopology,
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
    assert comp.kind == ComponentKind.Passive
    assert isinstance(comp.loc, Span)
    assert comp.loc.uri == "entities.fpp"


def test_component_ports(m):
    (comp,) = m.analysis.component_map.values()
    ports = comp.port_interface.port_map
    assert set(ports) == {"pIn", "pOut"}
    assert all(isinstance(p, (PortInstance, General)) for p in ports.values())
    dirs = {n: p.direction for n, p in ports.items()}
    assert dirs == {"pIn": Direction.Input, "pOut": Direction.Output}


def test_component_instances(m):
    a = m.analysis
    insts = {ci.qualified_name: ci for ci in a.component_instance_map.values()}
    assert set(insts) == {"a", "b"}
    assert insts["a"].base_id == 0x100
    assert insts["b"].base_id == 0x200
    # The instance's component symbol keys back into the component map.
    csym = insts["a"].component_symbol
    assert a.component_map[csym].node.name == "C"


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


def test_span_resolves(m):
    (comp,) = m.analysis.component_map.values()
    span = comp.loc
    loc = span.resolve()
    assert loc.uri == "entities.fpp"
    # `passive component C` is on the 3rd source line (0-indexed 2).
    assert span.line == loc.line == 2
    # Spans are hashable, handle-equal values.
    assert span == comp.loc and hash(span) == hash(comp.loc)
