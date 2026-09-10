"""AST traversal: the `AstNode.children` primitive and the `NodeVisitor` base class.

`NodeVisitor` is the Python counterpart of `fpp_ast::Visitor`. Because Python has
inheritance, `super().visit_<TypeName>(node)` plays the role of the Rust trait's
explicit `node.walk(..)`, which makes traversal deep by default here (the Rust
default is shallow). These tests pin that contract down: what counts as a child,
deep-vs-pruned recursion, the `generic_visit` funnel, and early exit by raising.
"""

import fpp as f
from fpp import AstNode, DefComponent, NodeVisitor

MODULE_SRC = """
module M {
  constant a = 1 + 2 * 3
  array Arr = [4] U32
  type T
}
"""

# Mirrors fpp_analysis/tests/port_numbering/ok.fpp, trimmed to one connection.
# Exercises `Connection` and `PortInstanceIdentifier` — the two `shadowed` node
# types, whose generated `visit_*` methods take the `AstNode` base.
TOPOLOGY_SRC = """
port P

passive component C {
  output port pOut: P
  sync input port pIn: P
}

instance c1: C base id 0x100
instance c2: C base id 0x200

topology T {
  instance c1
  instance c2
  connections P {
    c1.pOut -> c2.pIn
  }
}
"""

# `add_state_enums` splices synthetic enum definitions into the AST before the
# walk, so the recorded child edges must stay consistent with the mutated tree.
STATE_MACHINE_SRC = """
module M {
  state machine SM {
    action a
    guard g
    signal s
    initial enter S1
    state S1 {
      on s enter S2
    }
    state S2 {
      entry do { a }
      initial enter S3
      state S3
    }
  }
}
"""


def _roots(model):
    return [node for unit in model.ast for node in unit.members]


def _model(src: str, uri: str = "t.fpp"):
    m = f.analyze(source=src, uri=uri)
    assert not m.has_errors, [d.message for d in m.diagnostics]
    return m


class _Trace(NodeVisitor):
    """Records every node the traversal reaches, via the `generic_visit` funnel."""

    def __init__(self):
        self.nodes = []

    def generic_visit(self, node):
        self.nodes.append(node)
        return super().generic_visit(node)


def _trace(model):
    v = _Trace()
    for root in _roots(model):
        v.visit(root)
    return v.nodes


def _type_names(nodes):
    return [type(n).__name__ for n in nodes]


# --------------------------------------------------------------------------
# children
# --------------------------------------------------------------------------


def test_children_are_the_declared_members_in_source_order():
    (mod,) = _roots(_model(MODULE_SRC))
    assert _type_names(mod.children) == ["DefConstant", "DefArray", "DefAbsType"]


def test_children_flatten_through_kind_enums():
    # `Expr` holds an `ExprKind`; the kind is transparent, so an Expr's children
    # are the sub-expressions inside it rather than the kind wrapper.
    (mod,) = _roots(_model(MODULE_SRC))
    expr = mod.children[0].value
    assert type(expr.kind).__name__ == "ExprBinop"
    left, right = expr.children
    assert _type_names([left, right]) == ["Expr", "Expr"]
    # 1 + (2 * 3): the right operand is itself a binop with two operands.
    assert _type_names(right.children) == ["Expr", "Expr"]
    assert left.children == []


def test_children_are_transparent_through_unions():
    # `ComponentMember` and `SpecPortInstance` are unions; the concrete node
    # shows up as the child, never a union wrapper.
    m = _model(TOPOLOGY_SRC, uri="top.fpp")
    (comp,) = [n for n in _roots(m) if type(n).__name__ == "DefComponent"]
    assert _type_names(comp.children) == [
        "SpecGeneralPortInstance",
        "SpecGeneralPortInstance",
    ]


def test_childless_nodes_return_an_empty_list():
    # `type T` collapses to a name only, so it has no child nodes.
    (mod,) = _roots(_model(MODULE_SRC))
    abs_type = mod.children[2]
    assert type(abs_type).__name__ == "DefAbsType"
    assert abs_type.children == []


def test_a_definition_name_is_not_a_child():
    # Names are bound as plain strings by the binding, not as `Ident` nodes, so
    # `children` mirrors the typed field getters rather than the Rust walk.
    (mod,) = _roots(_model(MODULE_SRC))
    assert mod.name == "M"
    assert "Ident" not in _type_names(mod.children)


def test_children_preserve_node_identity():
    (mod,) = _roots(_model(MODULE_SRC))
    assert mod.children[0] is mod.children[0]
    assert mod.children[0] is mod.members[0]


# --------------------------------------------------------------------------
# NodeVisitor dispatch
# --------------------------------------------------------------------------


def test_traversal_is_deep_by_default():
    # A visitor overriding nothing but the funnel still reaches every nested
    # node, depth-first and in source order.
    assert _type_names(_trace(_model(MODULE_SRC))) == [
        "DefModule",
        # constant a = 1 + 2 * 3 -> 1, 2, 3, 2*3, 1+(2*3)
        "DefConstant", "Expr", "Expr", "Expr", "Expr", "Expr",
        # array Arr = [4] U32 -> the size expression, then the element type
        "DefArray", "Expr", "TypeName",
        "DefAbsType",
    ]  # fmt: skip


def test_override_receives_the_concrete_node_type():
    class Collect(NodeVisitor):
        def __init__(self):
            self.names = []

        def visit_DefComponent(self, node):
            assert isinstance(node, DefComponent)
            assert isinstance(node, AstNode)
            self.names.append(node.name)
            super().visit_DefComponent(node)

    v = Collect()
    for root in _roots(_model(TOPOLOGY_SRC, uri="top.fpp")):
        v.visit(root)
    assert v.names == ["C"]


def test_super_call_descends_and_omitting_it_prunes():
    class Deep(NodeVisitor):
        def __init__(self):
            self.ports = []

        def visit_DefComponent(self, node):
            super().visit_DefComponent(node)

        def visit_SpecGeneralPortInstance(self, node):
            self.ports.append(node.name)

    class Pruned(Deep):
        def visit_DefComponent(self, node):
            pass  # no super() call: the component's subtree is skipped

    m = _model(TOPOLOGY_SRC, uri="top.fpp")
    deep, pruned = Deep(), Pruned()
    for root in _roots(m):
        deep.visit(root)
        pruned.visit(root)
    assert deep.ports == ["pOut", "pIn"]
    assert pruned.ports == []


def test_generic_visit_override_makes_the_pass_shallow():
    # `generic_visit` is the single funnel every base `visit_*` delegates to, so
    # overriding it without calling super stops all recursion — the analogue of
    # the Rust trait's default (shallow) `super_visit`.
    class Shallow(NodeVisitor):
        def __init__(self):
            self.hits = []

        def generic_visit(self, node):
            self.hits.append(type(node).__name__)

    (mod,) = _roots(_model(MODULE_SRC))
    v = Shallow()
    v.visit(mod)
    assert v.hits == ["DefModule"]


def test_shallow_pass_descends_via_the_base_generic_visit():
    # Because every base `visit_*` routes through `self.generic_visit`, a shallow
    # `generic_visit` override also stops `super().visit_<TypeName>(node)`. To
    # descend one level anyway, call the base funnel directly. This is how the
    # Rust trait's shallow-pass idiom (override the containers, walk explicitly)
    # translates.
    class Selective(NodeVisitor):
        def __init__(self):
            self.hits = []

        def generic_visit(self, node):
            self.hits.append(type(node).__name__)  # never descend

        def visit_DefModule(self, node):
            super().generic_visit(node)  # ...except through a module

    (mod,) = _roots(_model(MODULE_SRC))
    v = Selective()
    v.visit(mod)
    assert v.hits == ["DefConstant", "DefArray", "DefAbsType"]


def test_raising_aborts_the_traversal():
    class Found(Exception):
        def __init__(self, node):
            self.node = node

    class Find(NodeVisitor):
        def __init__(self):
            self.visited = []

        def generic_visit(self, node):
            self.visited.append(type(node).__name__)
            return super().generic_visit(node)

        def visit_DefArray(self, node):
            raise Found(node)

    (mod,) = _roots(_model(MODULE_SRC))
    v = Find()
    try:
        v.visit(mod)
    except Found as e:
        assert e.node.name == "Arr"
    else:
        raise AssertionError("expected the visitor to raise")
    # The exception unwound the whole traversal: nothing after the array — which
    # is where it was raised — was reached.
    assert "DefAbsType" not in v.visited
    assert len(v.visited) < len(_trace(_model(MODULE_SRC)))


def test_shadowed_node_types_are_visitable():
    # `Connection` and `PortInstanceIdentifier` collide with semantic-layer class
    # names, so their generated methods take the `AstNode` base. Dispatch is by
    # runtime class name, so they still resolve, and super() still descends.
    class Conns(NodeVisitor):
        def __init__(self):
            self.connections = 0
            self.endpoints = []

        def visit_Connection(self, node):
            self.connections += 1
            super().visit_Connection(node)

        def visit_PortInstanceIdentifier(self, node):
            self.endpoints.append(node.port_name.data)
            super().visit_PortInstanceIdentifier(node)

    v = Conns()
    for root in _roots(_model(TOPOLOGY_SRC, uri="top.fpp")):
        v.visit(root)
    assert v.connections == 1
    assert v.endpoints == ["pOut", "pIn"]


def test_subclass_may_define_its_own_init_signature():
    # The base `__new__` swallows any arguments, so a subclass needs no
    # `super().__init__()` call and may take whatever arguments it likes.
    class Tagged(NodeVisitor):
        def __init__(self, tag, *, limit=0):
            self.tag = tag
            self.limit = limit

    v = Tagged("hello", limit=3)
    assert (v.tag, v.limit) == ("hello", 3)
    (mod,) = _roots(_model(MODULE_SRC))
    assert v.visit(mod) is None


def test_plain_visitor_traverses_without_error():
    # Every node type has a base `visit_*`, so an un-overridden visitor is a
    # complete no-op walk.
    v = NodeVisitor()
    for src, uri in (
        (MODULE_SRC, "t.fpp"),
        (TOPOLOGY_SRC, "top.fpp"),
        (STATE_MACHINE_SRC, "sm.fpp"),
    ):
        for root in _roots(_model(src, uri=uri)):
            assert v.visit(root) is None


def test_visit_rejects_a_non_node():
    try:
        NodeVisitor().visit("not a node")
    except TypeError:
        pass
    else:
        raise AssertionError("expected a TypeError")


# --------------------------------------------------------------------------
# The two traversals agree
# --------------------------------------------------------------------------


def _children_closure(roots):
    """Every node reachable through `children`, keyed by node id."""
    seen, stack = {}, list(roots)
    while stack:
        node = stack.pop()
        if node.node_id in seen:
            continue
        seen[node.node_id] = node
        stack.extend(node.children)
    return seen


def test_visitor_covers_exactly_the_children_closure():
    for src, uri in (
        (MODULE_SRC, "t.fpp"),
        (TOPOLOGY_SRC, "top.fpp"),
        (STATE_MACHINE_SRC, "sm.fpp"),
    ):
        m = _model(src, uri=uri)
        visited = _trace(m)
        closure = _children_closure(_roots(m))
        assert len(visited) == len(closure), uri
        # Each node is reached exactly once — no node is walked twice.
        assert len({n.node_id for n in visited}) == len(visited), uri
        assert {n.node_id for n in visited} == set(closure), uri


def test_state_machine_synthetic_nodes_are_reachable():
    # `add_state_enums` injects an enum definition for the machine's states; the
    # recorded child edges include it.
    seen = _type_names(_trace(_model(STATE_MACHINE_SRC, uri="sm.fpp")))
    assert {"DefStateMachine", "DefState", "DefEnum", "DefEnumConstant"} <= set(seen)
