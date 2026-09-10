# Codegen wrapper for the `fpp_python` crate.
#
# The two generators are plain `cargo run`s, but each needs a specific feature
# set — and `stub_gen` needs a linker hint on distros that ship libpython
# without the `-dev` package — so the invocations live here instead of in
# muscle memory. The "Codegen drift" job in `.github/workflows/python.yml` calls
# these targets rather than re-spelling the commands, so CI and a local run
# cannot diverge; that job adds only the pinned-nightly install and a GitHub
# annotation on the drift failure.
#
#   make              # = make codegen
#   make codegen      # regenerate both declarations, then the type stub
#   make bindgen      # src/ast/defs.rs + src/sem/defs.rs   (pinned nightly)
#   make bindgen-ast  # src/ast/defs.rs only                (stable only)
#   make bindgen-sem  # src/sem/defs.rs only                (pinned nightly)
#   make stubs        # fpp.pyi
#   make drift        # fail if any generated file differs from the index
#   make ext          # build the import-only extension (what the wheel ships)
#   make nightly      # install the pinned nightly the bindgen rustdoc needs
#
# Overridable: CARGO, PYTHON, CARGO_FLAGS (e.g. `make stubs CARGO_FLAGS=--release`).

CARGO ?= cargo
PYTHON ?= python3
CARGO_FLAGS ?=

# This Makefile's directory (`fpp_python/`) and the workspace root above it.
CRATE_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
ROOT := $(abspath $(CRATE_DIR)/..)

# Checked-in generated files, workspace-root-relative for `git diff`.
GENERATED := \
	fpp_python/src/ast/defs.rs \
	fpp_python/src/sem/defs.rs \
	fpp_python/fpp.pyi

# --- libpython linker hint (stub_gen only) ----------------------------------
#
# `stub_gen` is a standalone executable, so it is built with `extension-module`
# OFF and really does link libpython: pyo3 emits `-lpython3.X -L <LIBDIR>`. That
# resolves against the UNVERSIONED `libpython3.X.so` symlink, which ships in
# `python3-dev` — without it the link fails with `unable to find library
# -lpython3.X` even though `libpython3.X.so.1.0` is right there. CPython's own
# config dir (`LIBPL`) carries an equivalent symlink, so point the linker at it.
#
# Probed once per make run, and only used when the LIBDIR symlink is genuinely
# missing: RUSTFLAGS participates in cargo's fingerprint, so setting it
# unconditionally would force a rebuild whenever you alternate between `make`
# and a bare `cargo run`. Pass PYTHON= to match whatever interpreter pyo3
# resolves (it takes `python3` from PATH unless PYO3_PYTHON is set).
PY_CFG := $(shell $(PYTHON) -c 'import sysconfig as s; print(s.get_config_var("LIBDIR") or "", s.get_config_var("LDLIBRARY") or "", s.get_config_var("LIBPL") or "")' 2>/dev/null)
PY_LIBDIR := $(word 1,$(PY_CFG))
PY_LDLIBRARY := $(word 2,$(PY_CFG))
PY_LIBPL := $(word 3,$(PY_CFG))

ifeq ($(wildcard $(PY_LIBDIR)/$(PY_LDLIBRARY)),)
ifneq ($(PY_LIBPL),)
STUBGEN_ENV := RUSTFLAGS="-L $(PY_LIBPL)"
endif
endif

# Recipes run from the workspace root. `cargo run` finds the workspace from any
# subdirectory, but the generator it spawns inherits make's CWD — so this keeps
# every relative path (and `git diff` below) anchored regardless of whether you
# invoke `make` here or `make -C fpp_python` from elsewhere.
# The generator is a separate crate on purpose (see fpp_python_bindgen), so it
# stays buildable when a stale declaration has broken THIS crate.
BINDGEN := cd $(ROOT) && $(CARGO) run -p fpp_python_bindgen $(CARGO_FLAGS)
STUBGEN := cd $(ROOT) && $(STUBGEN_ENV) $(CARGO) run -p fpp_python $(CARGO_FLAGS) --no-default-features --features stubgen --bin stub_gen

# The exact nightly whose rustdoc JSON schema matches the generator's
# `rustdoc-types` pin. Single-sourced from the file the bindgen itself reads (and
# the CI job installs), so none of the three can drift apart.
NIGHTLY := $(shell tr -d '[:space:]' < $(ROOT)/fpp_python_bindgen/nightly-toolchain)

.DEFAULT_GOAL := codegen
.PHONY: codegen bindgen bindgen-ast bindgen-sem stubs drift ext nightly help

# Declarations first, then the stub: the stub is dumped from the pyclasses the
# declarations expand into, so a stub built before them reflects the old model.
# Sequential even under `make -j`, hence the recursive invocations.
codegen:
	$(MAKE) -f $(lastword $(MAKEFILE_LIST)) bindgen
	$(MAKE) -f $(lastword $(MAKEFILE_LIST)) stubs

# Full run: reflects `fpp_analysis` through the PINNED nightly's rustdoc (see
# `nightly` below). Not any nightly — rustdoc's JSON schema is unstable, and the
# generator asserts the `format_version` it emits against its `rustdoc-types` pin.
bindgen:
	$(BINDGEN)

# `--only ast` skips the rustdoc reflection (no nightly needed) and reuses the
# committed `shadowed {…}`/`leaves {…}` blocks, which only a full run derives.
bindgen-ast:
	$(BINDGEN) -- --only ast

bindgen-sem:
	$(BINDGEN) -- --only sem

stubs:
	$(STUBGEN)

drift:
	cd $(ROOT) && git diff --exit-code -- $(GENERATED) \
		|| { echo "error: checked-in declaration/stub is stale — run \`make -C fpp_python codegen\` and commit the result"; exit 1; }

# Proves the regenerated code still compiles in the configuration the wheel
# ships (`extension-module` on, Python symbols resolved by the host at import).
ext:
	cd $(ROOT) && $(CARGO) build -p fpp_python $(CARGO_FLAGS) --lib

# One-time setup for `bindgen`/`bindgen-sem`. `--profile minimal` is enough:
# rustdoc ships inside the `rustc` component.
nightly:
	rustup toolchain install $(NIGHTLY) --profile minimal

# Echoes the target list from the header comment. Matched by prefix, not by line
# number: the previous `sed -n '9,18p'` silently printed the wrong window every
# time a line was added above it.
help:
	@grep -E '^#   ' $(lastword $(MAKEFILE_LIST)) | sed 's/^# \{0,1\}//'
