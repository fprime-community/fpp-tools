"""The two entry points: `parse` (syntax only) and `analyze` (full pipeline)."""

import pytest

import fpp_python as f
from fpp_python import Model, SyntaxTree, TransUnit

COMMANDS = "tests/commands/commands.fpp"
EVENTS = "tests/events/events.fpp"

SHARED_SRC = "module Shared {\n  constant WIDTH = 8\n}\n"
USER_SRC = "module User {\n  constant total = Shared.WIDTH * 4\n}\n"

SM_SRC = """
state machine SM {
  signal x
  initial enter S
  state S {
    on x enter T
  }
  state T
}
"""


def _roots(units):
    return [node for unit in units for node in unit.members]


# --- shapes ----------------------------------------------------------------


def test_parse_returns_a_syntax_tree():
    st = f.parse(source=SHARED_SRC, uri="shared.fpp")
    assert isinstance(st, SyntaxTree)
    assert not st.has_errors and st.error_count == 0 and st.diagnostics == []
    (unit,) = st.units
    assert isinstance(unit, TransUnit)
    assert unit.uri == "shared.fpp"
    (mod,) = unit.members
    assert type(mod).__name__ == "DefModule" and mod.name == "Shared"


def test_analyze_returns_a_model():
    m = f.analyze(source=SHARED_SRC, uri="shared.fpp")
    assert isinstance(m, Model)
    (unit,) = m.ast
    assert isinstance(unit, TransUnit) and unit.uri == "shared.fpp"


def test_a_syntax_tree_exposes_no_semantics():
    st = f.parse(source=SHARED_SRC)
    assert not hasattr(st, "analysis")
    assert not hasattr(st, "lookup")


def test_unit_len_and_repr():
    st = f.parse(source=SHARED_SRC + USER_SRC, uri="both.fpp")
    (unit,) = st.units
    assert len(st) == 1 and len(unit) == 2 == len(unit.members)
    assert "both.fpp" in repr(unit) and "SyntaxTree" in repr(st)


def test_units_are_memoized():
    st = f.parse(source=SHARED_SRC)
    assert st.units[0] is st.units[0]
    m = f.analyze(source=SHARED_SRC)
    assert m.ast[0] is m.ast[0]
    assert m.ast[0].members[0] is m.ast[0].members[0]


# --- inputs ----------------------------------------------------------------


def test_a_bare_string_is_a_path_not_source():
    for entry in (f.parse, f.analyze):
        result = entry(COMMANDS)
        units = result.units if entry is f.parse else result.ast
        assert [u.uri for u in units] == [COMMANDS]


def test_a_list_of_paths_gives_one_unit_each_in_order():
    st = f.parse([EVENTS, COMMANDS])
    assert [u.uri for u in st.units] == [EVENTS, COMMANDS]
    assert not st.has_errors, [d.message for d in st.diagnostics]


def test_paths_and_source_combine_paths_first():
    m = f.analyze([COMMANDS], source=SHARED_SRC, uri="shared.fpp")
    assert [u.uri for u in m.ast] == [COMMANDS, "shared.fpp"]
    assert not m.has_errors, [d.message for d in m.diagnostics]


def test_no_input_is_a_value_error():
    for entry in (f.parse, f.analyze):
        with pytest.raises(ValueError):
            entry()


def test_an_unreadable_path_is_an_os_error():
    for entry in (f.parse, f.analyze):
        with pytest.raises(OSError):
            entry("tests/nonexistent.fpp")


# --- multi-unit analysis ---------------------------------------------------


def test_units_are_analyzed_together(tmp_path):
    a = tmp_path / "a.fpp"
    b = tmp_path / "b.fpp"
    a.write_text(SHARED_SRC)
    b.write_text(USER_SRC)
    m = f.analyze([str(a), str(b)])
    assert not m.has_errors, [d.message for d in m.diagnostics]
    assert [[n.name for n in u.members] for u in m.ast] == [["Shared"], ["User"]]
    # `User.total` folds only because `Shared.WIDTH`, in the *other* unit, resolved.
    assert m.lookup("User.total").definition.value.resolved_value.value == 32


# --- what each phase does --------------------------------------------------


def test_parse_resolves_includes():
    # A syntactic splice, so it happens at parse depth too.
    st = f.parse("tests/includes/host.fpp")
    assert not st.has_errors, [d.message for d in st.diagnostics]
    (host,) = st.units[0].members
    (included,) = host.members
    assert type(included).__name__ == "DefConstant" and included.name == "included"
    assert included.location.uri.endswith("tests/includes/inc.fppi")


def test_parse_reports_syntax_errors():
    st = f.parse(source="module M { constant = }")
    assert st.has_errors and st.error_count >= 1
    assert st.diagnostics[0].level == "error"


def test_only_analyze_applies_the_state_enum_transform():
    parsed = f.parse(source=SM_SRC, uri="sm.fpp")
    analyzed = f.analyze(source=SM_SRC, uri="sm.fpp")
    assert not analyzed.has_errors, [d.message for d in analyzed.diagnostics]
    (parsed_sm,) = parsed.units[0].members
    (analyzed_sm,) = analyzed.ast[0].members
    assert "DefEnum" not in [type(n).__name__ for n in parsed_sm.members]
    assert [type(n).__name__ for n in analyzed_sm.members][0] == "DefEnum"


def test_parsed_nodes_have_locations_but_no_resolved_semantics():
    st = f.parse(source="module M {\n  constant a = 1\n  constant b = a + 1\n}\n")
    (mod,) = _roots(st.units)
    a, b = mod.members
    assert a.location.line == 1 and a.node_id != b.node_id
    assert [type(n).__name__ for n in mod.children] == ["DefConstant", "DefConstant"]
    assert b.value.resolved_value is None
    assert b.value.resolved_type is None
    use = b.value.kind.left  # the `a` use site
    assert type(use.kind).__name__ == "ExprIdent" and use.kind.value == "a"
    assert use.definition is None


def test_analyze_resolves_what_parse_leaves_unresolved():
    m = f.analyze(source="module M {\n  constant a = 1\n  constant b = a + 1\n}\n")
    (mod,) = _roots(m.ast)
    b = mod.members[1]
    assert b.value.resolved_value.value == 2
    use = b.value.kind.left
    assert m.analysis.get_qualified_name(use.definition) == "M.a"
