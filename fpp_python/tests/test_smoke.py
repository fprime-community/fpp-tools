"""Smoke tests for the native binding entry point and the `Analysis` root."""

import fpp as f
from fpp import Analysis


def test_analyze_ok():
    m = f.analyze(source="module M {\n  constant a = 1234\n}\n")
    assert not m.has_errors and m.error_count == 0 and m.diagnostics == []
    (unit,) = m.ast
    (mod,) = unit.members
    assert type(mod).__name__ == "DefModule" and mod.name == "M"
    (const,) = mod.members
    assert type(const).__name__ == "DefConstant" and const.name == "a"
    assert type(const.value.kind).__name__ == "ExprLiteralInt"
    assert const.value.kind.value == "1234"


def test_analyze_reports_errors():
    m = f.analyze(source="module M { constant x = nope }")
    assert m.has_errors and len(m.diagnostics) >= 1
    d = m.diagnostics[0]
    assert d.level is f.DiagnosticLevel.Error and d.location is not None


def test_node_identity_is_memoized():
    m = f.analyze(source="module M { constant a = 1 }")
    (unit,) = m.ast
    (mod,) = unit.members
    # Repeated navigation returns the same Python object.
    assert mod.members[0] is mod.members[0]
    assert m.ast[0].members[0] is mod


def test_analysis_root_is_exposed():
    # `model.analysis` is the 1:1 mirror of `fpp_analysis::Analysis`; its public
    # maps are real Python dicts and its query methods are callable.
    m = f.analyze(source="module M {\n  constant a = 1\n  constant b = a + 1\n}\n")
    assert not m.has_errors, [d.message for d in m.diagnostics]
    a = m.analysis
    assert isinstance(a, Analysis)
    # A node-id-keyed dict of resolved values (b folds, a folds).
    assert len(a.value_map) >= 2
    # The two constants resolve to symbols in the symbol map.
    qnames = {a.get_qualified_name(s) for s in a.symbol_map.values()}
    assert {"M.a", "M.b"} <= qnames
    # `lookup` + `get_qualified_name` agree.
    sym = m.lookup("M.a")
    assert sym is not None and a.get_qualified_name(sym) == "M.a"
