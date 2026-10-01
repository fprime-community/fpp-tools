"""Type-resolution tests over the `Type` union mirror of
`fpp_analysis::semantics::Type`.

Entities are located by qualified name via `Model.lookup`; their resolved type
is read through `symbol.definition.resolved_type`. `Type` is the base class, each
variant is a nested subclass of it (`Type.Array`), and `Type.Variant` is the
closed union over them. Public native fields are getters (`Type.Array.anon_array`,
`Type.Enum.rep_type`) and `&self`/`&Arc<Self>` methods are getters too
(`is_int`, `underlying_type`).
"""

import pytest

import fpp as f
from fpp import (
    Symbol,
    Type,
)
from fpp.ast import IntegerKind

SRC = """
module M {
  array Arr = [4] U32
  enum E: I32 { A, B }
  struct S { x: U32, y: F32 }
  type Alias = U16
  type A2 = Arr
}
"""


@pytest.fixture(scope="module")
def m():
    model = f.analyze(source=SRC, uri="types.fpp")
    assert not model.has_errors, [d.message for d in model.diagnostics]
    return model


def rtype(m, qn):
    return m.lookup(qn).definition.resolved_type


def test_array_type(m):
    t = rtype(m, "M.Arr")
    assert isinstance(t, Type.Array)
    assert isinstance(t, Type)
    # `Type.Array.anon_array` rewraps into the structural anonymous-array type.
    aa = t.anon_array
    assert isinstance(aa, Type.AnonArray)
    assert aa.size == 4
    assert isinstance(aa.elt_type, Type.PrimitiveInt)
    assert aa.elt_type.value == IntegerKind.U32
    # The `node` field bridges to the defining AST wrapper; a symbol's `node_id`
    # is that node's id.
    assert t.node.node_id == m.lookup("M.Arr").node_id


def test_enum_type(m):
    t = rtype(m, "M.E")
    assert isinstance(t, Type.Enum)
    assert t.rep_type == IntegerKind.I32
    assert t.is_displayable is True


def test_struct_type(m: f.Model):
    t = rtype(m, "M.S")
    assert isinstance(t, Type.Struct)
    anon = t.anon_struct
    assert [name for name, _ in anon.members] == ["x", "y"]
    assert isinstance(anon.get_member("x"), Type.PrimitiveInt)
    assert isinstance(anon.get_member("y"), Type.Float)


def test_alias_type(m):
    t = rtype(m, "M.Alias")
    assert isinstance(t, Type.Alias)
    # `alias_type` is the immediate aliased type; `underlying_type` (a base method
    # getter) follows the alias chain. Both resolve to U16 here.
    assert isinstance(t.alias_type, Type.PrimitiveInt)
    assert t.alias_type.value == IntegerKind.U16
    assert isinstance(t.underlying_type, Type.PrimitiveInt)
    assert t.underlying_type.value == IntegerKind.U16


def test_type_predicates(m):
    assert rtype(m, "M.Alias").is_int is True
    assert rtype(m, "M.E").is_int is False
    assert rtype(m, "M.Arr").is_numeric is False


def test_symbols_are_union_subclasses(m):
    arr = m.lookup("M.Arr")
    assert isinstance(arr, Symbol.ArrayType)
    assert isinstance(arr, Symbol)
    assert isinstance(m.lookup("M.E"), Symbol.EnumType)


def test_type_map_is_populated(m):
    tm = m.analysis.type_map
    assert tm  # node-id-keyed dict of resolved types
    assert all(isinstance(k, int) for k in tm)
    assert all(isinstance(v, Type) for v in tm.values())
    # The array's resolved type is registered under its def node id.
    arr_node = m.lookup("M.Arr").node_id
    assert isinstance(tm[arr_node], Type.Array)


def test_symbol_value_equality(m):
    assert m.lookup("M.Arr") == m.lookup("M.Arr")
