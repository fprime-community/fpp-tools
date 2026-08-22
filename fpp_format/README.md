# fprime-fpp-format

A formatter for the F Prime Prime (FPP) modeling language.

This package ships the `fpp-format` executable, a pretty-printer for `.fpp`
source files.

## Usage

```sh
# Format files in place
fpp-format path/to/model.fpp

# Check formatting without modifying (exit 1 if not formatted)
fpp-format --check path/to/model.fpp

# Format from stdin to stdout
cat path/to/model.fpp | fpp-format --stdin

# Override the indentation width and maximum line length
fpp-format --indent 2 --line-length 100 path/to/model.fpp
```

## Options

| Option           | Description                                                        |
| ---------------- | ------------------------------------------------------------------ |
| `--check`              | Check formatting without writing; exit `1` if a file is unformatted. |
| `--stdin`              | Read from stdin and write to stdout (default when no files given). |
| `--recursive-includes` | Also follow `include` specifiers and format reachable `.fppi` fragments. |
| `--indent <N>`         | Number of spaces per indentation level. Overrides `.fpp-format` (default: `4`). |
| `--line-length <N>`    | Maximum line width before specs explode their clauses. Overrides `.fpp-format` (default: `80`). |
| `--entry <RULE>`       | Select the parser entrypoint / grammar rule (see below).          |
| `--help`               | Print usage.                                                       |

## Configuration file (`.fpp-format`)

The indentation width and maximum line length can be set project-wide in a
`.fpp-format` file. Starting from each formatted file's directory, `fpp-format`
searches upward through parent directories for the nearest `.fpp-format` file
(the same discovery model as `.clang-format`). The same file is honored by
`fprime-util format` and by the language server's format-on-save, so the editor
and CI always agree.

The file is a minimal `key = value` list; blank lines and `#` comments are
ignored:

```ini
# .fpp-format
indent = 4
line-length = 80
```

Supported keys:

| Key           | Description                              | Default |
| ------------- | ---------------------------------------- | ------- |
| `indent`      | Spaces per indentation level.            | `4`     |
| `line-length` | Maximum line width before clauses break. | `80`    |

Precedence, lowest to highest: built-in defaults → `.fpp-format` file →
`--indent` / `--line-length` command-line flags. A malformed `.fpp-format` file
is a hard error on the command line (to avoid formatting with the wrong profile
and reporting false `--check` failures); the language server logs it and falls
back to defaults so an editor save never fails.

## Entrypoint rule (`--entry`)

The formatter parses input starting from a specific grammar rule. A whole
`.fpp` file is a `module` (the default), but other rules are needed when
formatting **include fragments**.

Supported rules:

- `module` (default)
- `component`
- `topology`
- `tlm-packet`
- `tlm-packet-set`

## Recursive include formatting (`--recursive-includes`)

By default `fpp-format` formats only the files you name and does **not** touch
`.fppi` fragments referenced via `include`. Pass `--recursive-includes` to also
follow every `include` specifier and format the referenced fragments
(recursively):

```sh
# Formats model.fpp and every .fppi it (transitively) includes
fpp-format --recursive-includes path/to/model.fpp
```

With `--recursive-includes`, include paths are resolved relative to the
including file (matching the parser), each fragment is formatted exactly once
even if included from several places, and `include` cycles are detected and
reported as an error. The entrypoint for each fragment is **derived from the
context of its `include`** — a fragment included in a `component { ... }` body is
formatted as component members, one in a `topology { ... }` body as topology
members, and so on, so you do not need `--entry` for fragments reached this way.

State-machine includes have no standalone entrypoint and are left untouched.

Note: the language-server integration never follows includes; it only ever
formats the current document buffer.

## Formatting a bare `.fppi` file

An `.fppi` file is not a standalone module — it is a fragment that is spliced in
via an `include` specifier. When you format a fragment **directly** (rather than
reaching it from a root `.fpp`), there is no include context to infer the rule
from, so pass `--entry` to select the matching entrypoint:

```sh
# An .fppi included inside a `module { ... }` body (module-level members)
fpp-format --entry module commands.fppi

# An .fppi included inside a `component { ... }` body
fpp-format --entry component ports.fppi

# An .fppi included inside a `topology { ... }` body
fpp-format --entry topology connections.fppi
```

If you omit `--entry`, the `module` rule is used, which is correct for normal
`.fpp` files and for `.fppi` fragments containing module-level members.
