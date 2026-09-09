# fpp_python

Native Python bindings to the [FPP](https://nasa.github.io/fpp/) compiler. The
`fpp_python` extension binds **directly** to the Rust FPP compiler in-process via
PyO3: parsing and semantic analysis run in-process, and the AST and analysis are
exposed as a live, navigable Python object graph.

Because the binding holds the compiler's real in-memory model, cross-references
are exposed as **actual Python object references** (e.g. `use.definition`,
`instance.component`, `node.resolved_type`) rather than being flattened through
integer AST ids.

## Installation

```sh
pip install fprime-fpp-python
```

Building from source requires a Rust toolchain (edition 2024, i.e. Rust ≥ 1.85);
the wheel is built with [maturin](https://www.maturin.rs/) and ships an `abi3`
extension usable on CPython ≥ 3.10.

## Usage

```python
import fpp_python

model = fpp_python.analyze(source="""
module M {
  array Arr = [4] U32
  constant answer = 6 * 7
}
""")

if model.has_errors:
    for d in model.diagnostics:
        print(d.level, d.location, d.message)

# Navigate the AST: one TransUnit per input
(unit,) = model.ast
(module,) = unit.members
for member in module.members:
    print(type(member).__name__, getattr(member, "name", None))

# Resolve semantics by navigation (no AST ids)
arr = model.lookup("M.Arr")                 # -> Symbol (here an ArraySymbol)
t = arr.definition.resolved_type            # -> Type (here an ArrayType); t.array_size == 4
answer = model.lookup("M.answer").definition.value.resolved_value  # -> Value (an IntegerValue); .value == 42
```

### Entry points

Both entry points take the same inputs — a `.fpp` path, a list of paths, and/or
in-memory `source=` text — and produce one **translation unit** per input:

```python
fpp_python.analyze("Top.fpp")                     # one file
fpp_python.analyze(["A.fpp", "B.fpp"])            # analyzed together
fpp_python.analyze(source="constant a = 1")       # in-memory, uri="<string>"
fpp_python.analyze(["A.fpp"], source=src, uri="patch.fpp")   # both
```

A bare string is always a **path**, never source; pass `source=` for text. All
units given to one `analyze` call are analyzed together, so a definition in one
resolves uses in another. An unreadable path raises `OSError`.

`parse(...)` is the fast front end: it parses and resolves `include`s, then stops.
It returns a `SyntaxTree` — the units and the syntax diagnostics, and nothing
semantic. Use it when you only need syntax:

```python
tree = fpp_python.parse(["A.fpp", "B.fpp"])
for unit in tree.units:
    print(unit.uri, [type(m).__name__ for m in unit.members])
```

`analyze(...)` runs the whole pipeline and returns a `Model` carrying the
**transformed** AST — the parsed units after include resolution and the state-enum
transform — alongside the analysis computed from it. Nodes reached from a
`SyntaxTree` have locations, annotations, and children, but their `definition`,
`resolved_type`, and `resolved_value` are all `None`; only `analyze` fills those
in.

### Typed unions and enums

The "closed union" semantic types — `Symbol`, `Type`, `Value`, `PortInstance`,
`StateMachineElement` — are each a union of concrete subclasses over a base
class. A getter typed as `Type` returns one of `ArrayType | EnumType | …`;
discriminate with `isinstance` / `match` (each subclass exposes only its own
fields) rather than a string tag:

```python
from fpp_python import ArrayType, PrimitiveInt, IntegerKind

match arr.definition.resolved_type:
    case ArrayType() as a:
        elt = a.anon_array.elt_type     # -> Type union
        if isinstance(elt, PrimitiveInt) and elt.value == IntegerKind.U32:
            ...
```

Enum-valued fields are real Python enums (`IntegerKind`, `ComponentKind`,
`EventSeverity`, `QueueFull`, `Direction`, `CommandKind`, …), compared by member
(e.g. `component.kind == ComponentKind.Passive`).

A `Model` exposes:

- `model.ast` — the transformed AST: one `TransUnit` per input, in the order
  given. A unit has `.uri` and `.members` (its top-level definitions, in source
  order).
- `model.analysis` — the `Analysis` root, the 1:1 mirror of the compiler's
  semantic model; navigate it through its typed maps (`component_map`,
  `state_machine_map`, …) and methods (`get_qualified_name(sym)`).
- `model.diagnostics` / `model.has_errors` / `model.error_count` — structured
  diagnostics.
- `model.lookup(qualified_name)` — a `Symbol` by dotted name.

A `SyntaxTree` exposes `tree.units` plus the same `diagnostics` / `has_errors` /
`error_count`.

Every AST node exposes `.node_id`, `.location`, `.pre_annotation` /
`.post_annotation`, `.children`, and — where applicable — `.definition` (the
resolved symbol), `.resolved_type`, and `.resolved_value`. Node identity is
stable: navigating to the same node twice returns the same Python object.

### Walking the AST

`NodeVisitor` is the traversal counterpart of the compiler's `fpp_ast::Visitor`.
Subclass it and override `visit_<TypeName>` for the node types you care about,
where `<TypeName>` is the node's class name — the same string as
`type(node).__name__`. Each override receives its concrete node class, so the
fields you reach are fully typed.

```python
from fpp_python import NodeVisitor

class Constants(NodeVisitor):
    def __init__(self):
        self.values = {}

    def visit_DefConstant(self, node):
        # `node` is a DefConstant, so `.name` and `.value` are typed; the
        # folded value comes from the analysis.
        self.values[node.name] = node.value.resolved_value
        super().visit_DefConstant(node)      # keep descending

consts = Constants()
for unit in model.ast:
    for root in unit.members:
        consts.visit(root)

# module M { constant width = 8; constant total = width * 4 }
# -> {"width": 8, "total": 32}
print({name: v.value for name, v in consts.values.items()})
```

Traversal is depth-first, pre-order, in source order, and **deep by default** —
the inverse of the Rust trait, where recursion is opt-in via an explicit
`node.walk(..)`. Here the base `visit_<TypeName>` walks the children for you, so:

- Call `super().visit_<TypeName>(node)` to descend from an override; **omit it to
  prune** that subtree.
- Override `generic_visit(node)` to hook *every* node — it is the single funnel
  each base `visit_<TypeName>` delegates to, and the analogue of the Rust trait's
  `super_visit`. Returning from it without calling
  `super().generic_visit(node)` makes the whole pass shallow.
- Raise an exception to stop early; return values are not inspected.

`node.children` is the same traversal as a plain `list[AstNode]`, for one-off
queries that do not warrant a visitor class:

```python
kinds = [type(c).__name__ for c in component.children]
```

Children are the AST *nodes* reached through a node's fields: `kind` enums and
member unions are transparent (an `Expr`'s children are the sub-expressions
inside its `kind`), and fields the binding collapses to a plain value — such as a
definition's `name` — are not children. Traversal is read-only; the parsed AST is
immutable, so there is no counterpart to `fpp_ast::MutVisitor`.

## Development

The extension is a Cargo workspace member of
[fpp-tools](https://github.com/fprime-community/fpp-tools). The AST node wrappers
and the recording walk are expanded at compile time by the
`fpp_python_macros::fpp_ast_bindings!` proc macro from a checked-in declaration
(`src/ast/defs.rs`, a ~1:1 mirror of the `fpp_ast` grammar), as are the semantic
wrappers from `src/sem/defs.rs`; the small core (`pipeline`, `ir_core`,
`lower_core`, `noderef`, `model`, `visitor`, `diagnostics`) is hand-written.

```sh
maturin develop            # build + install the extension into the active venv
pytest tests/              # run the test suite

make                       # regenerate declarations, then the type stub
make help                  # list the individual codegen targets
```

`make` wraps the two generators (see the `Makefile` for the individual targets);
the underlying commands are:

```sh
# Regenerate the checked-in declarations after an `fpp_ast`/`fpp_analysis` change.
# A standalone crate, so a declaration left stale by an upstream change cannot
# block the build of the generator that fixes it:
cargo run -p fpp_python_bindgen

# Regenerate the type stub after changing the exposed API. Built WITHOUT
# `extension-module`, so it links libpython for real — on a distro without
# `python3-dev` this needs `RUSTFLAGS="-L $(python3 -c 'import sysconfig;
# print(sysconfig.get_config_var("LIBPL"))')"`, which `make stubs` adds for you:
cargo run -p fpp_python --no-default-features --features stubgen --bin stub_gen
```

Run the declaration generator before the stub dump — the stub is derived from the
pyclasses the declarations expand into. `make` and CI both enforce that order.
