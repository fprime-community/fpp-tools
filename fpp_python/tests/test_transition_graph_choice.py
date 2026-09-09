"""The `Choice` transition-graph arc exposes its `a_node` as an AST wrapper.

`TransitionGraphArc::Choice` carries a `std::sync::Arc<fpp_ast::TransitionExpr>`
`a_node`. `TransitionExpr` is a real recorded walk node, so the binding surfaces
it as `astdef(TransitionExpr)` — a downcast to the `crate::ast::TransitionExpr`
wrapper. This test navigates to a Choice arc and confirms `a_node` downcast
cleanly (did not raise, and did not fall back to the opaque base).
"""

import fpp_python as f
from fpp_python import StateMachineSymbol, TransitionGraphArcChoice

# A state machine whose initial transition enters a choice, and whose choice has
# guarded `if`/`else` transitions — so the transition graph records `Choice` arcs
# whose `a_node` is the `TransitionExpr` of each branch.
SM_SRC = """
module M {
  state machine SM {
    guard g
    state S
    initial enter C
    choice C { if g enter S else enter S }
  }
}
"""


def test_choice_arc_a_node_is_transition_expr():
    m = f.analyze(source=SM_SRC, uri="sm_choice.fpp")
    assert not m.has_errors, [d.message for d in m.diagnostics]

    (_sym, sm) = next(iter(m.analysis.state_machine_map.items()))
    tg = sm.sma.transition_graph

    # Collect every Choice arc reachable from the (node-keyed) arc map.
    choice_arcs = [
        arc
        for arcs in tg.arc_map.values()
        for arc in arcs
        if isinstance(arc, TransitionGraphArcChoice)
    ]
    assert choice_arcs, "the choice `C` should produce at least one Choice arc"

    for arc in choice_arcs:
        # The start of a choice arc is the choice symbol itself.
        assert isinstance(arc.start_choice, StateMachineSymbol)
        # `a_node` downcast to the AST wrapper: it must NOT have raised and must
        # NOT have come back as the opaque `AstNode` base.
        a_node = arc.a_node
        assert type(a_node).__name__ == "TransitionExpr", type(a_node).__name__
