# fprime-fpp-python

Native Python bindings to the [FPP](https://nasa.github.io/fpp/) compiler.
Installed as `fprime-fpp-python`, imported as `fpp`.

The extension binds directly to the Rust FPP compiler via PyO3.

## Installation

```sh
pip install fprime-fpp-python
```

An `abi3` wheel, usable on CPython ≥ 3.10. Building from source needs Rust ≥ 1.85
and [maturin](https://www.maturin.rs/).

## Usage

```python
import fpp

model = fpp.analyze(source="""
module M {
  array Arr = [4] U32
  constant answer = 6 * 7
}
""")

model.has_errors                             # False
for d in model.diagnostics:
    d.level                                  # fpp.DiagnosticLevel.Error
    print(d.display)                         # 'path:line:col: error: message'
    print(d)                                 # the compiler's console rendering

(unit,) = model.ast                          # one translation unit per input
(module,) = unit.members                     # fpp.ast.DefModule
[type(m).__name__ for m in module.members]   # ['DefArray', 'DefConstant']

arr = model.lookup("M.Arr")                                     # Symbol.ArrayType
arr.definition.resolved_type.array_size                         # 4
model.lookup("M.answer").definition.value.resolved_value.value  # 42
```

All inputs to one call are analyzed together. A bare string is a path, `source=`
is text, and `imports=` — the counterpart of `fpp-to-cpp -i` — is analyzed the
same way but is not part of what you asked about:

```python
model = fpp.analyze(["MyComponent.fpp"], imports=["Fw/Fw.fpp"])
[u.uri for u in model.ast if u.is_source]    # ['MyComponent.fpp']
```

`parse` is the fast front end: it stops after `include` resolution, so you get
syntax and nothing resolved.

```python
tree = fpp.parse(["A.fpp", "B.fpp"])
[u.uri for u in tree.units]                  # ['A.fpp', 'B.fpp']
```

The syntax tree lives in the `fpp.ast` submodule — one class per grammar
production, plus the `AstNode` base and the enums the grammar spells
(`fpp.ast.IntegerKind`, `fpp.ast.ComponentKind`). `fpp` itself is the analysis:
`Model`, `Analysis`, the symbol/type/value unions, and the semantic entities. The
split is what lets both keep the compiler's own names, since three of them belong
to each — `fpp.Connection` is the analyzed connection, `fpp.ast.Connection` the
syntax it came from.

```python
from fpp.ast import DefArray

isinstance(module.members[0], DefArray)      # True
type(module.members[0]).__module__           # 'fpp.ast'
```

Subclass `fpp.AstVisitor` — it takes AST nodes, but it is an entry point, so it
stays in `fpp` beside `analyze` — and override `visit_<type(node).__name__>`.
Traversal is deep by default: `super()` descends, omitting it prunes.

```python
class Constants(fpp.AstVisitor):
    def __init__(self):
        self.values = {}

    def visit_DefConstant(self, node):
        self.values[node.name] = node.value.resolved_value.value
        super().visit_DefConstant(node)

consts = Constants()
consts.visit(model)                          # or a SyntaxTree, TransUnit, or node
consts.values                                # {'answer': 42}
```

Semantic types are closed unions.

```python
match arr.definition.resolved_type:
    case fpp.Type.Array() as a:
        elt = a.anon_array.elt_type
        isinstance(elt, fpp.Type.PrimitiveInt) and elt.value == fpp.ast.IntegerKind.U32
```

Annotate with `<Base>.Variant` — `fpp.Type.Variant`, `fpp.Value.Variant` — rather
than the base class. It is the closed union over the variants, so a type checker can tell you which `case` you forgot.

```python
def describe(v: fpp.Value.Variant) -> str:
    match v:
        case fpp.Value.Integer():
            return f"integer {v.value}"
        case fpp.Value.String():
            return f"string {v.value}"
        ...
        case _:
            typing.assert_never(v)   # fails while any variant is unhandled
```

Findings of your own report like compiler errors: build a `Diagnostic` against
any node's `span`, add child annotations and notes, and `print` it.

```python
node = model.lookup("M.answer").definition
print(fpp.Diagnostic(
    "answer is unused", level=fpp.DiagnosticLevel.Warning,
    span=node.span,
    children=[fpp.DiagnosticMessage("delete it")],
))
#  --> mem.fpp:4:3
#   |
# 4 |   constant answer = 6 * 7
#   |   ^^^^^^^^^^^^^^^^^^^^^^^ answer is unused
#   |
#   = note: delete it
```

A check can raise its finding instead of returning it: `fpp.DiagnosticError`
carries a `Diagnostic`, and everything on that diagnostic stays writable, so each
handler on the way out can add the context it knows.

```python
try:
    check_component(instance.component)
except fpp.DiagnosticError as error:
    error.diagnostic.add_note("in this instance", span=instance.node.span)
    raise
```

## Development

A Cargo workspace member of
[fpp-tools](https://github.com/fprime-community/fpp-tools). The node wrappers and
the recording walk are expanded by `fpp_python_macros` from checked-in
declarations (`src/ast/defs.rs`, `src/sem/defs.rs`); the core (`pipeline`,
`ir_core`, `lower_core`, `noderef`, `model`, `visitor`, `diagnostics`) is
hand-written.

Those declarations and the two stubs are generated and checked in. Change the generator or the macro, never the file, and re-run `make`. CI fails on drift.

```sh
maturin develop            # build + install into the active venv
pytest tests/              # run the tests

make nightly               # one-time: the nightly the bindgen's rustdoc needs
make                       # regenerate the declarations, then the stubs
make help                  # the individual codegen targets
```

The bindgen reflects `fpp_analysis` from rustdoc JSON, whose schema is unstable,
so the nightly is pinned exactly — in `fpp_python_bindgen/nightly-toolchain`
alongside the `rustdoc-types` pin in its `Cargo.toml`. Bump the two together.
`FPP_BINDGEN_TOOLCHAIN` overrides the toolchain for a one-off run.
