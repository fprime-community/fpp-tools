"""Tests for the method-parameter argkinds the bindgen marshals.

Every `argkind` in the `fpp_sem_bindings!` vocabulary is exercised here through a
method that actually takes it, so a regression in the generated marshalling shows up
as a failure rather than as a silently re-skipped method:

* `node`                  -- a `fpp_core::Node`, fed from any `AstNode` wrapper
* `astnode(X)`            -- a live `fpp_ast::X`, fed from the `X` wrapper
* `span`                  -- a `fpp_core::Span`, fed from a `Span` wrapper
* `union(N)` / `entity(N)` -- a native semantic value, lent by its Python wrapper
* `arc(union(N))`         -- an `Arc`-stored native, lent without a clone
* `opt(..)` / `list(..)`  -- rebuilt containers, including the empty/`None` cases
* `throws`                -- a native `Result` error raised as `ValueError`

It also covers the two cross-cutting guarantees: a wrapper from a different `Model`
is rejected, and a field getter can coexist with a `get_<field>` method.
"""

import pytest

import fpp as f

SRC = """
constant SIZE = 4
array Arr = [SIZE] U32
module M {
  constant N = 3
  constant NEG = -2
  port P
  passive component C {
    sync input port pin: P
    output port pout: P
  }
  instance c1: C base id 0x100
  instance c2: C base id 0x200
  topology T {
    instance c1
    instance c2
    connections C1 { c1.pout -> c2.pin }
  }
}
"""


@pytest.fixture(scope="module")
def m():
    model = f.analyze(source=SRC, uri="params.fpp")
    assert not model.has_errors, [d.message for d in model.diagnostics]
    return model


@pytest.fixture(scope="module")
def other():
    """A second, independent model — for the cross-model argument guard."""
    model = f.analyze(
        source="port Q\npassive component D { sync input port qin: Q }\n",
        uri="other.fpp",
    )
    assert not model.has_errors, [d.message for d in model.diagnostics]
    return model


def walk(node, out):
    out.append(node)
    for child in node.children:
        walk(child, out)
    return out


@pytest.fixture(scope="module")
def nodes(m):
    out = []
    for unit in m.ast:
        for member in unit.members:
            walk(member, out)
    return out


def of_kind(nodes, kind):
    return [n for n in nodes if type(n).__name__ == kind]


@pytest.fixture(scope="module")
def a_span(m):
    """Any real `Span`, reached through a semantic getter."""
    (top,) = m.analysis.topology_map.values()
    (conn,) = top.connection_map["C1"]
    return conn.from_.loc


# --- `node`: a fpp_core::Node fed from an AstNode wrapper --------------------


def test_node_param_from_ast_wrapper(m, nodes):
    exprs = of_kind(nodes, "Expr")
    assert exprs, "the source has constant expressions"
    folded = {v for v in (m.analysis.get_int_value(e) for e in exprs) if v is not None}
    # SIZE = 4, M.N = 3, M.NEG = -2 (and the `2` inside it).
    assert {4, 3, -2}.issubset(folded)


def test_node_param_accepts_any_node_subclass(m, nodes):
    """The param is typed to the shared `AstNode` base, so any node is accepted."""
    array_def = of_kind(nodes, "DefArray")[0]
    # A DefArray is not a use-site with an int value, but it marshals fine.
    assert m.analysis.get_int_value(array_def) is None


# --- `astnode(X)` inside `opt(..)` -------------------------------------------


def test_opt_astnode_param_some_and_none(m, nodes):
    size_expr = of_kind(nodes, "DefArray")[0].size
    assert m.analysis.get_big_int_value_opt(size_expr) == 4
    assert m.analysis.get_big_int_value_opt(None) is None


def test_opt_astnode_param_rejects_wrong_node_type(m, nodes):
    """`astnode(Expr)` is typed to `Expr` exactly, so PyO3 rejects other nodes."""
    with pytest.raises(TypeError):
        m.analysis.get_big_int_value_opt(of_kind(nodes, "DefArray")[0])


# --- `span` + `throws` ------------------------------------------------------


def test_span_param_and_ok_result(m, nodes, a_span):
    size_expr = of_kind(nodes, "DefArray")[0].size
    assert m.analysis.get_array_size(size_expr, a_span) == 4


def test_throws_raises_value_error(m, nodes, a_span):
    neg = next(e for e in of_kind(nodes, "Expr") if m.analysis.get_int_value(e) == -2)
    with pytest.raises(ValueError):
        m.analysis.get_nonnegative_int_value(neg, a_span)


def test_throws_unit_ok_returns_none(m):
    """A `Result<(), E>` return is `None` on success."""
    port_interface = next(iter(m.analysis.component_map.values())).port_interface
    assert port_interface.implements(port_interface) is None


# --- `entity(N)` ------------------------------------------------------------


@pytest.fixture(scope="module")
def topology(m):
    return next(iter(m.analysis.topology_map.values()))


@pytest.fixture(scope="module")
def connection(topology):
    return next(iter(topology.connection_map.values()))[0]


def endpoints(connections):
    """`Connection` carries no identity directive, so compare by endpoint name."""
    return [
        (c.from_.port.qualified_name, c.to.port.qualified_name) for c in connections
    ]


def test_entity_param(topology, connection):
    src, dst = connection.from_.port, connection.to.port
    wired = (src.qualified_name, dst.qualified_name)
    assert endpoints(topology.get_connections_from(src)) == [wired]
    assert endpoints(topology.get_connections_to(dst)) == [wired]
    assert endpoints(topology.get_connections_at(src)) == [wired]
    assert endpoints(topology.get_connections_between(src, dst)) == [wired]
    assert topology.connection_exists_between(src, dst)
    assert not topology.connection_exists_between(dst, src)
    # An unconnected identifier yields nothing, exercising the empty-result path.
    assert topology.get_connections_from(dst) == []


# --- `union(N)` and `list(entity(N))` ---------------------------------------


@pytest.fixture(scope="module")
def port_instances(m):
    comp = next(iter(m.analysis.component_map.values()))
    return list(comp.port_interface.port_map.values())


def test_union_param(port_instances):
    first, second = port_instances[0], port_instances[1]
    assert first.signature_eq(first)
    assert not first.signature_eq(second)


def test_union_param_rejects_other_hierarchy(port_instances, types):
    """A `union(..)` param accepts any member of *its* union and nothing else.

    The parameter renders as the union alias in the stub, but it still extracts a
    `PyRef` of that union's base class, so PyO3 rejects a wrapper from another
    hierarchy.
    """
    with pytest.raises(TypeError):
        port_instances[0].signature_eq(types[0])


def test_union_and_list_entity_params(topology, connection, port_instances):
    pi = port_instances[0]
    assert topology.get_port_number(pi, connection) == 0
    assert topology.get_used_port_numbers(pi, [connection]) == [0]
    # The empty list must marshal too (a `Vec` built from no elements).
    assert topology.get_used_port_numbers(pi, []) == []


def test_list_astnode_param(m):
    """`&[fpp_ast::FormalParam]`: the Vec is rebuilt from the supplied wrappers."""
    ports = f.analyze(source="module M { port P(a: U32, b: U8) }")
    assert not ports.has_errors, [d.message for d in ports.diagnostics]
    nodes = []
    for unit in ports.ast:
        for member in unit.members:
            walk(member, nodes)
    params = of_kind(nodes, "FormalParam")
    assert len(params) == 2
    assert ports.analysis.check_displayable_params(params, "ctx") is None
    # The empty list must marshal too (a `Vec` built from no elements).
    assert m.analysis.check_displayable_params([], "ctx") is None


# --- `arc(union(Type))` and `union(Value)` ----------------------------------


@pytest.fixture(scope="module")
def types(m):
    return list(m.analysis.type_map.values())


def test_arc_union_param_is_reflexive(types):
    assert types[0].identical(types[0])


def test_arc_union_param_common_type(types):
    """`common_type` takes two `&Arc<Type>`s and yields one."""
    assert types[0].common_type(types[0]) is not None


def test_union_value_param(m):
    """`Value::convert` takes `&Arc<Type>` — converting to its own type is a no-op."""
    value = next(iter(m.analysis.value_map.values()))
    converted = value.convert(value.type)
    assert converted is not None
    assert converted.type.identical(value.type)


# --- cross-cutting guarantees ----------------------------------------------


def test_cross_model_argument_is_rejected(m, other, nodes):
    """A handle only means something inside its own model, so mixing is an error."""
    expr = of_kind(nodes, "Expr")[0]
    with pytest.raises(ValueError, match="different model"):
        other.analysis.get_int_value(expr)


def test_cross_model_entity_argument_is_rejected(m, other):
    mine = next(iter(m.analysis.component_map.values())).port_interface
    theirs = next(iter(other.analysis.component_map.values())).port_interface
    with pytest.raises(ValueError, match="different model"):
        mine.implements(theirs)


def test_field_getter_and_get_field_method_coexist(m):
    """`Analysis.interface` (a field) and `Analysis.get_interface(node)` (a method).

    PyO3 derives a getter's symbol from its Rust ident and a method's from its Python
    name, so these two collide unless the generator renames one; both must survive.
    """
    analysis = m.analysis
    assert analysis.interface is None or analysis.interface is not None  # readable
    assert callable(analysis.get_interface)
    assert analysis.component_instance is None or True
    assert callable(analysis.get_component_instance)
