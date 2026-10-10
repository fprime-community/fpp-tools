"""Module templates, end to end.

Expanding a template belongs to the analysis, not to parsing: `expand T` names a
template, and resolving that name is a symbol lookup. So `analyze` splices a copy
of the template body in at the specifier and the copies are the definitions the
rest of the analysis resolves, while `parse` leaves the specifier alone.

What a consumer walking the model needs from that is asserted here: the expansion
contributed real definitions whose values came from the bound arguments, the copies
are distinct from the template body they came from, each argument is readable
through the expansion that bound it, and a diagnostic against a copy says which
expansion made it — which is the only thing distinguishing two copies of one line.
"""

import fpp as f

# A constant parameter and a type parameter, expanded once. `width` folds from the
# bound `4` and `Frame.body` resolves to the bound `Telem`, so what each argument
# contributed is observable in the analyzed model.
SRC = """\
module M {
  module template Scaled(constant factor: U32, type Payload) {
    constant width = factor * 8
    struct Frame { body: Payload }
  }

  struct Telem { x: U32 }

  expand Scaled(constant 4, type Telem)
}
"""

# The same template expanded twice, into two different modules, with two different
# arguments.
TWICE_SRC = """\
module M {
  module template Scaled(constant factor: U32) {
    constant width = factor * 8
  }

  expand Scaled(constant 4)
}

module N {
  expand M.Scaled(constant 10)
}
"""

# An instance parameter: the topology in the body instantiates whatever component
# instance the expansion is given. `c` offers a superset of `I` — an `extra` port the
# interface does not declare — so which of the two the bound parameter answers with
# is observable.
INSTANCE_SRC = """\
port P

interface I {
  sync input port pIn: P
  output port pOut: P
}

passive component C {
  sync input port pIn: P
  output port pOut: P
  output port extra: P
}

instance c: C base id 1

module template Wrap(instance i: I) {
  topology Top {
    instance i
  }
}

expand Wrap(instance c)
"""

# Two expansions of a template that defines `a`, so both copies define it in the
# same scope. Both the primary annotation and its child then point at the one
# `constant a` line, and only the expansion each came from tells them apart.
COLLIDING_SRC = """\
module M {
  module template T {
    constant a = 1
  }
  expand T
  expand T
}
"""


def analyzed(source: str = SRC):
    m = f.analyze(source=source)
    assert not m.has_errors, [d.display for d in m.diagnostics]
    return m


def one_expansion(units):
    """The one module of a one-unit model, and the one `expand` specifier in it."""
    (module,) = units[0].members
    (spec,) = [x for x in module.members if isinstance(x, f.SpecTemplateExpand)]
    return module, spec


def test_an_expansion_contributes_definitions_bound_to_its_arguments():
    m = analyzed()
    # `width` exists only because the template was expanded, and its value is the
    # body's `factor * 8` with `factor` bound to the argument.
    width = m.lookup("M.width", kind=f.SymbolConstant)
    assert width is not None
    assert width.definition.value.resolved_value.value == 32
    # The type argument flowed the same way: the struct member declared as the
    # parameter `Payload` resolves to the `Telem` that was passed.
    frame = m.lookup("M.Frame", kind=f.SymbolStructType)
    body = m.analysis.type_map[frame.node_id].anon_struct.get_member("body")
    assert body.def_symbol == m.lookup("M.Telem")


def test_the_expanded_body_hangs_off_the_specifier():
    m = analyzed()
    module, spec = one_expansion(m.ast)
    (template,) = [x for x in module.members if isinstance(x, f.DefModuleTemplate)]
    body = [type(x).__name__ for x in template.members]
    assert body == ["DefConstant", "DefStruct"]
    # The copy is the same syntax with its own node identities, so a walk sees the
    # body twice: once as the template's, once as this expansion's.
    assert [type(x).__name__ for x in spec.members] == body
    assert {x.node_id for x in spec.members}.isdisjoint(
        {x.node_id for x in template.members}
    )
    # The definitions the analysis resolved are the copies, not the template's own —
    # and only the copies are symbols, so a name in a template body is looked up
    # once per expansion of it and never for the body itself.
    assert m.lookup("M.width").node_id == spec.members[0].node_id
    assert m.lookup("M.Frame").node_id == spec.members[1].node_id
    assert len(m.lookup_all("M.width")) == 1


def test_a_copy_keeps_the_location_of_the_body_it_came_from():
    m = analyzed()
    module, spec = one_expansion(m.ast)
    (template,) = [x for x in module.members if isinstance(x, f.DefModuleTemplate)]
    # A copy points at the text a reader has to look at to change it, which is the
    # template body — above the specifier, not at it.
    for copy, original in zip(spec.members, template.members):
        assert copy.location.line == original.location.line
        assert copy.location.line < spec.location.line


def test_parse_leaves_the_specifier_unexpanded():
    tree = f.parse(source=SRC)
    assert not tree.has_errors, [d.display for d in tree.diagnostics]
    _, spec = one_expansion(tree.units)
    # Nothing resolved the template name, so there is no body to splice in — but
    # the specifier itself is fully parsed.
    assert spec.members is None
    assert spec.template.data == "Scaled"
    assert [type(a.kind).__name__ for a in spec.args] == [
        "TemplateArgConstant",
        "TemplateArgType",
    ]


def test_a_template_definition_is_its_own_symbol_kind():
    m = analyzed()
    template = m.lookup("M.Scaled", kind=f.SymbolTemplate)
    assert template is not None
    assert isinstance(template.definition, f.DefModuleTemplate)
    assert [(type(p).__name__, p.name) for p in template.definition.params] == [
        ("TemplateParamConstant", "factor"),
        ("TemplateParamType", "Payload"),
    ]


def test_the_expansion_reports_the_argument_bound_to_each_parameter():
    m = analyzed()
    module, spec = one_expansion(m.ast)
    (template,) = [x for x in module.members if isinstance(x, f.DefModuleTemplate)]
    expansion = m.analysis.template_expansion_map[spec.node_id]
    assert expansion.expansion is spec and expansion.def_node is template
    assert m.analysis.expansion_at(spec).def_node is template

    factor, payload = expansion.params
    assert isinstance(factor, f.SymbolTemplateConstantArg)
    assert factor.param_def.name == "factor"
    assert factor.value.kind.value == "4"
    assert isinstance(payload, f.SymbolTemplateTypeArg)
    assert payload.param_def.name == "Payload"
    # A bound parameter is not a global definition, so it is a symbol that says so.
    assert all(p.is_template_arg for p in expansion.params)
    assert m.analysis.template_expansion_params(spec) == [factor, payload]


def test_two_expansions_of_one_template_are_two_sets_of_definitions():
    m = analyzed(TWICE_SRC)
    assert m.lookup("M.width").definition.value.resolved_value.value == 32
    assert m.lookup("N.width").definition.value.resolved_value.value == 80
    assert len(m.analysis.template_expansion_map) == 2


def test_an_instance_argument_becomes_an_instance_of_the_topology():
    m = analyzed(INSTANCE_SRC)
    (spec,) = [x for x in m.ast[0].members if isinstance(x, f.SpecTemplateExpand)]
    (bound,) = m.analysis.template_expansion_map[spec.node_id].params
    assert isinstance(bound, f.SymbolTemplateInterfaceArg)
    assert bound.param_def.name == "i" and bound.is_interface_instance

    topology = m.analysis.topology_map[m.lookup("Top", kind=f.SymbolTopology)]
    (instance,) = topology.instance_map
    # The topology instantiates the parameter, so the instance is the parameter —
    # carrying the component instance the expansion passed for it.
    assert isinstance(instance, f.TemplateArgInterfaceInstance)
    assert instance.qualified_name == "i"
    assert isinstance(instance.instance, f.ComponentInterfaceInstance)
    assert instance.instance.node.node_id == m.lookup("c").node_id
    # Only the interface the parameter declared is visible inside the expansion, so
    # the ports of the instance are that interface's — `c`'s `extra` is not among
    # them, even though `c` is what was passed.
    assert instance.declared_interface.unqualified_name == "I"
    assert sorted(instance.interface.port_map) == ["pIn", "pOut"]
    assert sorted(instance.instance.interface.port_map) == ["extra", "pIn", "pOut"]
    assert list(topology.direct_template_args) == [bound]


def test_a_diagnostic_in_an_expansion_names_the_expansion_that_made_it():
    m = f.analyze(source=COLLIDING_SRC)
    (redefinition,) = [d for d in m.diagnostics if d.level is f.DiagnosticLevel.Error]
    (previous,) = redefinition.children
    # Both point at the one line the template body has, so `location` alone cannot
    # tell a consumer which copy each is about.
    assert redefinition.location.line == previous.location.line
    assert redefinition.source == previous.source == "    constant a = 1\n"
    # `expansions` is what does: the second `expand T` made the copy being
    # reported, the first made the one it collides with.
    (second,) = redefinition.expansions
    (first,) = previous.expansions
    assert (second.line, second.column) == (5, 2)
    assert (first.line, first.column) == (4, 2)
    # Neither is inside an `include`, and both notes are in the rendering.
    assert redefinition.includes == previous.includes == []
    assert f"expanded at {second.display}" in str(redefinition)
    assert f"expanded at {first.display}" in str(redefinition)
