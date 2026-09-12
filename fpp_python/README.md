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
(module,) = unit.members
[type(m).__name__ for m in module.members]   # ['DefArray', 'DefConstant']

arr = model.lookup("M.Arr")                                     # SymbolArrayType
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

Subclass `NodeVisitor` and override `visit_<type(node).__name__>`. Traversal is
deep by default: `super()` descends, omitting it prunes.

```python
class Constants(fpp.NodeVisitor):
    def __init__(self):
        self.values = {}

    def visit_DefConstant(self, node):
        self.values[node.name] = node.value.resolved_value.value
        super().visit_DefConstant(node)

consts = Constants()
consts.visit(model)                          # or a SyntaxTree, TransUnit, or node
consts.values                                # {'answer': 42}
```

Semantic types are closed unions, so narrow them with `isinstance` or `match`
rather than a string tag.

```python
match arr.definition.resolved_type:
    case fpp.ArrayType() as a:
        elt = a.anon_array.elt_type
        isinstance(elt, fpp.PrimitiveIntType) and elt.value == fpp.IntegerKind.U32
```

Findings of your own report like compiler errors: build a `Diagnostic` against
any node's `span`, add child annotations and notes, and `print` it.

```python
node = model.lookup("M.answer").definition
print(fpp.Diagnostic(
    fpp.DiagnosticLevel.Warning, "answer is unused",
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

`fpp.pyi` is the reference for the rest — every class, getter and return type —
and the docstrings carry the contracts: `help(fpp.analyze)`, `help(fpp.Model)`,
`help(fpp.NodeVisitor)`.

## Development

A Cargo workspace member of
[fpp-tools](https://github.com/fprime-community/fpp-tools). The node wrappers and
the recording walk are expanded by `fpp_python_macros` from checked-in
declarations (`src/ast/defs.rs`, `src/sem/defs.rs`); the core (`pipeline`,
`ir_core`, `lower_core`, `noderef`, `model`, `visitor`, `diagnostics`) is
hand-written.

Those declarations and `fpp.pyi` are generated and checked in — change the
generator or the macro, never the file, and re-run `make`. CI fails on drift.

```sh
maturin develop            # build + install into the active venv
pytest tests/              # run the tests

make nightly               # one-time: the nightly the bindgen's rustdoc needs
make                       # regenerate the declarations, then the stub
make help                  # the individual codegen targets
```

The bindgen reflects `fpp_analysis` from rustdoc JSON, whose schema is unstable,
so the nightly is pinned exactly — in `fpp_python_bindgen/nightly-toolchain`
alongside the `rustdoc-types` pin in its `Cargo.toml`. Bump the two together.
`FPP_BINDGEN_TOOLCHAIN` overrides the toolchain for a one-off run.
