# fpp-query

List the files an FPP autocoder would generate, from the syntactic model alone.

An F Prime build autocoder is invoked twice: once at CMake **configure** time to
declare which files it will generate, and once at **build** time to generate them.
The configure-time query runs once per module, needs only the syntax model, and in
a Python autocoder is dominated by interpreter and extension-module startup.

On a project with a few hundred `.fpp` files that is tens of seconds of serial
configure time.

```sh
fpp-query --rules static-tlm-packet.toml \
          -d "$BUILD" --filenames "$BUILD/names.txt" -- Top/topology.fpp
```

```toml
# static-tlm-packet.toml
[[group]]
node = "DefTopology"
where = '$.is_deployment'
generate = ["StaticTlmPacketAc.cpp"]
```

## The rules file

The file holds the **rules**; the command line holds the **invocation** — `-d`,
`--filenames`, the input files — because those are build paths that change per
module while the rules do not.

A file is a list of groups. Each emits `<DIR>/<stem><SUFFIX>` for every definition
of `node` that satisfies `where`. Output is sorted and deduplicated across all
groups.

| Key        | Meaning                                                |
| ---------- | ------------------------------------------------------ |
| `node`     | AST node kind to select, e.g. `DefTopology`. Required. |
| `where`    | Predicate the definition must satisfy.                 |
| `name`     | Expression overriding the file stem.                   |
| `generate` | Suffixes appended to the stem. Required, non-empty.    |

One file-wide key sits above the first `[[group]]`:

| Key      | Meaning                                            |
| -------- | -------------------------------------------------- |
| `expand` | Match against the expanded model. Default `false`. |

**Write a query as a TOML literal string** — `where = '$.is_deployment'`, in single
quotes. A query needs `"` for its own string literals, and a literal string carries
those unescaped. A basic string with escapes in it is rejected rather than
mis-reported: its decoded text is shorter than what is written, so every caret past
the first escape would point at the wrong column.

A misspelled key names the alternatives exactly (`unknown field \`wheer\`, expected
one of \`node\`, \`where\`, \`name\`, \`generate\``), and every diagnostic — a bad
kind, a bad suffix, a query that will not parse — carets the offending value at its
real line in the file. One run reports every rule the file gets wrong.

## The command line

```
fpp-query [OPTIONS] --rules <FILE> [--] [FILES]...
```

| Option                    | Meaning                                                     |
| ------------------------- | ----------------------------------------------------------- |
| `--rules <FILE>`          | The TOML rules. Required, except for `--json` / `--fields`. |
| `-d`, `--directory <DIR>` | Directory the paths are rooted at. Default `.`.             |
| `--filenames <FILE>`      | Write the paths to `FILE` instead of stdout.                |
| `-i`, `--imports <FILES>` | Accepted and ignored, so one argv works for both phases.    |
| `--json`                  | Dump the serialized model — exactly what a query sees.      |
| `--fields [KIND]`         | List every kind, or one kind's queryable fields.            |

That is the whole surface. Rules only ever come from a file, so there are no group
flags to order, no shell quoting to get right around a `$`, and nothing to reconcile
when a flag and a file disagree. The trade is that a one-off query needs a file too;
`--json` and `--fields` are the exploratory modes that do not.

`--json` and `--fields` also accept `--rules`, which is how you see the model a
particular rule set reads — including its `expand` setting.

## The stem

By default the stem is the definition's name, prefixed recursively by the names of
enclosing components and state machines. Module nesting contributes nothing. This
is what `fpp-to-cpp` does, so the defaults reproduce its filenames:

```fpp
module M {
  passive component C {
    array A = [3] U32         # C_AArrayAc.hpp
    state machine SM {
      array A = [2] U8        # C_SM_AArrayAc.hpp
      initial enter S
      state S
    }
  }
  array A = [4] U8            # AArrayAc.hpp -- module M is not part of the name
  deployment topology T { }   # TTopologyAc.hpp
}
```

## Reproducing `fpp-filenames`

Upstream FPP ships an `fpp-filenames` tool with these rules built in. All six of its
modes are expressible here, and ship as presets:

| Preset                                    | Upstream                          |
| ----------------------------------------- | --------------------------------- |
| `presets/autocode.toml`                   | no flags                          |
| `presets/autocode-expanded.toml`          | no flags, from the expanded model |
| `presets/template.toml`                   | `-t`                              |
| `presets/test.toml`                       | `-u`                              |
| `presets/test-auto-helpers.toml`          | `-u -a`                           |
| `presets/test-template.toml`              | `-u -t`                           |
| `presets/test-template-auto-helpers.toml` | `-u -t -a`                        |

```sh
fpp-query --rules presets/autocode.toml -d "$OUT" -- "$@"
```

Copy the file into your project and delete the groups you do not want. Its comments
explain each rule, and it is the tested spelling of them: `tests/filenames.rs` runs
these presets against upstream's own suite — the models in `tests/filenames/` and the
`.ref.txt` reference outputs are copied verbatim from the Scala compiler's
`compiler/tools/fpp-filenames/test` — so the file you copy is the one held to
upstream's output byte for byte, not a transcription of it.

## The query language

A query is evaluated against one definition at a time. Values are the JSON that
`fpp_ast`'s `Serialize` impls produce — there is no separate reflection layer to
drift out of step with the grammar, so **a field is queryable exactly when it is
serialized**. Run `--json` to see it, or `--fields <KIND> <FILES>` for one node.

Every definition has an `annotations` field — `{"pre": [...], "post": [...]}`,
never omitted, empty arrays when there is none — because annotations live in the
compiler context keyed by node handle, not as a struct field, and would otherwise
be invisible to `--json` and to `$.field` navigation. It reaches further than the
`$@pre`/`$@post` roots below: those only ever read the top-level matched node,
while `$.members[0].DefComponent.annotations.pre` reaches a *nested* one's.

### Roots

| Root                                      | Value                                                     |
| ----------------------------------------- | --------------------------------------------------------- |
| `$`                                       | the matched definition                                    |
| `$.field`, `$.a.b`, `$.members[0]`        | field navigation                                          |
| `$.annotations.pre`, `$.annotations.post` | the matched definition's own annotations, as a real field |
| `$@`                                      | every annotation line, `@` then `@<`                      |
| `$@pre`, `$@post`                         | just one side                                             |
| `$@kind`                                  | the node kind name, e.g. `"DefTopology"`                  |
| `$@file`, `$@line`                        | source file URI, 1-based start line                       |
| `$@included`                              | true when spliced in by an `include`                      |
| `$@scope`, `$@qualified`                  | dotted enclosing scope; scope and name                    |
| `$@stem`                                  | the default filename stem                                 |
| `$^`, `$^Kind`                            | immediate parent; nearest enclosing `Kind`                |

### Operators

`==` `!=` `<` `<=` `>` `>=` · `contains` `starts_with` `ends_with` `matches` (glob:
`*`, `?`, `\`) · `in` · `+` (concatenation) · `&&` `||` `!` · `len()` `lower()`
`upper()` `join()` `replace()` · `"str"` `'str'` `42` `true` `false` `null` `[a, b]`

A **list on the left** of `contains` / `starts_with` / `ends_with` / `matches` holds
when any element does. `==` deliberately does not lift, so use `in` for exact
membership.

The evaluator is strict: no truthiness, no cross-type comparison, and navigating
into an absent field is an error rather than silently `null`. A query that quietly
evaluated to `false` would drop a file from the list, and a build output that was
declared but never written is far harder to diagnose than an error here. Guard
optional fields with `!= null` — `&&` short-circuits.

### Checking for an annotation

```toml
where = '$@ contains "static-tlm-packetizer"'   # any line containing the tag
where = '"static-tlm-packetizer" in $@'         # one line that is exactly the tag
where = 'len($@) > 0'                           # any annotation at all
where = 'len($@) == 0'                          # none
where = '$@pre matches "static-*"'              # only a `@` line, matched as a glob
```

Annotation text is what the lexer stores: the `@` or `@<` sigil is stripped and the
line trimmed, one list element per line.

### More examples

```toml
where = '$.kind in ["Active", "Queued"]'   # active or queued components
where = '$.members != null'                # bodied state machines only
where = '!$@included'                      # written here, not pulled in by an include

# Arrays declared inside a state machine
[[group]]
node = "DefArray"
where = '$^DefStateMachine != null'
generate = ["ArrayAc.hpp"]

# Telemetry packet sets, whose name depends on the enclosing topology
[[group]]
node = "SpecTlmPacketSet"
where = '$^DefTopology.is_deployment'
name = '$^DefTopology.name + "_" + $.name'
generate = ["TlmPacketsAc.hpp", "TlmPacketsAc.cpp"]

# Collapse many definitions onto one pair of files
[[group]]
node = "DefConstant"
name = '"FppConstants"'
generate = ["Ac.hpp", "Ac.cpp"]
```

## CMake integration

Replace the configure-time half of an autocoder. The `fpp_info` /
`fpp_autocoder_variables` calls can be dropped from that half entirely: they exist
to produce `FPP_IMPORT_FLAGS`, which a syntax-only query ignores, and they require
the `fpp_depend` sub-build cache to already exist — itself configure-time cost.

```cmake
function(static_tlm_packet_setup_autocode MODULE_NAME AC_INPUT_FILES)
    set(NAMES "${CMAKE_CURRENT_BINARY_DIR}/static-tlm-packet-filenames.txt")
    set(RULES "${CMAKE_CURRENT_LIST_DIR}/static-tlm-packet.toml")
    # Editing the rules must re-run configure, or the declared output list goes
    # stale and Ninja reports an output that was never produced.
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${RULES}")

    execute_process_or_fail(
        "[static_tlm_packet] could not list generated files for ${MODULE_NAME}"
        "${FPP_QUERY}"
        "--rules" "${RULES}"
        "-d" "${CMAKE_CURRENT_BINARY_DIR}"
        "--filenames" "${NAMES}"
        "--" ${AC_INPUT_FILES}
    )
    file(STRINGS "${NAMES}" GENERATED_CPP)

    if (NOT GENERATED_CPP)
        # Defined-but-empty, not undefined: the autocoder contract requires one of
        # the AUTOCODER_GENERATED_* variables to exist in the caller's scope.
        set(AUTOCODER_GENERATED_BUILD_SOURCES "" PARENT_SCOPE)
        return()
    endif()
    set(AUTOCODER_GENERATED_BUILD_SOURCES "${GENERATED_CPP}" PARENT_SCOPE)

    fpp_info("${MODULE_NAME}" "${AC_INPUT_FILES}")
    fpp_autocoder_variables("${FPP_IMPORTS}")
    add_custom_command(
        OUTPUT ${GENERATED_CPP}
        COMMAND ${STATIC_TLM_PACKETIZER} "-d" "${CMAKE_CURRENT_BINARY_DIR}"
            ${FPP_IMPORT_FLAGS} ${AC_INPUT_FILES}
        DEPENDS ${FILE_DEPENDENCIES} "${STATIC_TLM_PACKETIZER}"
        COMMENT "Generating telemetry packet code for ${MODULE_NAME}"
    )
endfunction()
```

The generator and the query must agree on the path set exactly, or Ninja reports a
declared output that was never produced. This is what the rules file is for: it is
one artifact both halves can read, so the build-time generator can derive its own
output names from it — `tomllib.load` in a Python autocoder — rather than keeping a
second copy of the same suffixes in sync by hand.

### Exit codes and output format

| Code | Meaning                                                                                   |
| ---- | ----------------------------------------------------------------------------------------- |
| 0    | Success. The `--filenames` file exists, possibly empty. Nothing on stdout or stderr.      |
| 1    | Diagnostics were emitted: a syntax error, an unresolvable `include`, or a bad rules file. |
| 2    | Usage or I/O failure, including an unreadable `--rules` file.                             |

**No matches is not a failure.** A nonzero exit becomes a CMake `FATAL_ERROR` that
aborts configure, and a module with nothing to generate must not do that.

The `--filenames` file is written **unconditionally**, including on exit 1, because
`file(STRINGS)` on a missing file is a hard `FATAL_ERROR` — leaving it out would turn
one diagnosable error into two. It holds one absolute path per line, LF-terminated,
with no blank lines (a blank line becomes an empty CMake list element, hence an
empty `add_custom_command(OUTPUT ...)` entry). Diagnostics go to **stderr**, since
stdout is where the list goes when `--filenames` is absent.
