"""Value-resolution tests over the `Value` union mirror of
`fpp_analysis::semantics::Value`.

Constants are located by qualified name; their folded value is read through
`def_constant.value.resolved_value`. Native tuple-struct variants render as
`Value*` subclasses (`Integer` -> `ValueInteger`, etc.) to stay clear of the
`enum.Enum`-style names.
"""

import pytest

import fpp_python as f
from fpp_python import (
    AnonArrayValue,
    AnonStructValue,
    EnumConstantValue,
    EnumType,
    FloatValue,
    Value,
    ValueBoolean,
    ValueInteger,
    ValueString,
)

SRC = """
module M {
  constant c = 1 + 2 * 3
  constant s = "hello"
  constant b = true
  constant fl = 2.5
  constant arr = [1, 2, 3]
  constant st = { x = 1, y = 2 }
  enum E: I32 { A, B }
  constant ec = E.B
}
"""


@pytest.fixture(scope="module")
def m():
    model = f.analyze(source=SRC, uri="values.fpp")
    assert not model.has_errors, [d.message for d in model.diagnostics]
    return model


def rval(m, qn):
    return m.lookup(qn).definition.resolved_value


def test_integer_constant_folding(m):
    v = rval(m, "M.c")
    assert isinstance(v, ValueInteger) and isinstance(v, Value)
    assert v.value == 7  # 1 + 2*3
    # `get_type` is mirrored as the `.type` getter (PyO3 strips the `get_` prefix).
    assert v.type.is_int


def test_string(m):
    v = rval(m, "M.s")
    assert isinstance(v, ValueString) and v.value == "hello"


def test_bool(m):
    v = rval(m, "M.b")
    assert isinstance(v, ValueBoolean) and v.value is True


def test_float(m):
    v = rval(m, "M.fl")
    assert isinstance(v, FloatValue) and abs(v.value - 2.5) < 1e-9


def test_anon_array(m):
    v = rval(m, "M.arr")
    assert isinstance(v, AnonArrayValue)
    assert [e.value for e in v.elements] == [1, 2, 3]


def test_anon_struct(m):
    v = rval(m, "M.st")
    assert isinstance(v, AnonStructValue)
    assert {k: mv.value for k, mv in v.members.items()} == {"x": 1, "y": 2}


def test_enum_constant(m):
    v = rval(m, "M.ec")
    assert isinstance(v, EnumConstantValue)
    # `EnumConstantValue.value` is the native `(String, i128)` tuple.
    assert v.value == ("B", 1)
    assert isinstance(v.ty, EnumType)


def test_value_map_is_populated(m):
    vm = m.analysis.value_map
    assert vm  # node-id-keyed dict of folded values
    assert all(isinstance(k, int) for k in vm)
    assert all(isinstance(v, Value) for v in vm.values())
    # The `c` constant's folded value is registered under its expression node id.
    c_expr_node = m.lookup("M.c").definition.value.node_id
    assert vm[c_expr_node].value == 7
