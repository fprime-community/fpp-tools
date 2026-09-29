# VSCode-FPP

VSCode extension for FPP Language Support.

[FPP](https://github.com/nasa/fpp) is a modeling language for the [F Prime flight software framework](https://github.com/nasa/fprime).

## Get Started

The extension needs the `fpp_lsp_server` executable and a valid F´ build cache.

### Language server

Install the language server into your project's Python virtual environment:

```bash
pip install fprime-fpp-lsp
```

When you open a `.fpp` file, the extension locates `fpp_lsp_server` automatically:

1. the `fpp.serverPath` setting, if you set it explicitly;
2. the workspace Python venv — discovered via the [Python extension][py-ext] or by
   scanning for a `fprime-venv`/`.venv`/`venv`/`env` directory (override with `fpp.pythonVenv`);
3. `fpp_lsp_server` on your `PATH`.

If a venv is found but `fprime-fpp-lsp` is not installed, the extension offers to run
`pip install fprime-fpp-lsp` for you and then starts the server.

The **FPP Language Server** status item (in the language status menu, next to the FPP
Project item) shows which binary was detected, its version, and where it came from. Click
**Change** (or run **FPP: Select Language Server**) to browse for a specific executable or
return to auto-discovery.

For venv installs, the extension periodically checks PyPI for a newer `fprime-fpp-lsp` and
offers to update it in place. Disable this with `fpp.checkForUpdates`, or trigger it
manually with **FPP: Check for Language Server Update**.

[py-ext]: https://marketplace.visualstudio.com/items?itemName=ms-python.python

### Build cache

1. Run `fprime-util generate`
2. Open a `.fpp` file. The project should start indexing and references should resolve.


### Troubleshooting
When first loading up your F Prime project, you may notice errors. This is because the compiler doesn't know
where to search for FPP declarations.

The language server reads project configuration from a **`.fpp-lsp`** file at the
workspace root (see [`docs/fpp-lsp-config.md`](../../docs/fpp-lsp-config.md)). It
tells the server which `locs.fpp` to index — this file is generated during build
time in the cmake build folder, e.g. `build-fprime-automatic-native/locs.fpp`. If
you do not have a `build-fprime-automatic-native/` folder, run `fprime-util generate`.

You can create/update `.fpp-lsp` from the editor: run **FPP: Select Locs file inside
workspace** (`fpp.select`) and pick the discovered `locs.fpp` (or choose to scan the
whole workspace). The extension writes your choice into `.fpp-lsp` and the server
re-indexes automatically. You can also edit `.fpp-lsp` by hand — the server picks up
changes on save.

An example `.fpp-lsp`:

```yaml
# Point the server at the locs file inside your F´ build cache.
buildCache: build-fprime-automatic-native
```

It's recommended to 'pin' the FPP project status item so it's easy to reload/reindex
the project.

## Features

- Syntax highlighting
- Code completion
  - Syntax level completion
  - Semantic specific identifier lookup
    - When searching for a type, only types will be shown. Same goes for ports, components etc.
- Syntax Signature Display
  - This should pop up while you are typing but can be manually triggered, see [VSCode Docs](https://code.visualstudio.com/docs/typescript/typescript-editing#_signature-help)
  ![Screenshot from 2023-06-20 15-28-08](https://github.com/user-attachments/assets/250a4c93-ed79-41df-9fe1-14aec397f133)
  - Includes descriptions on what each field does
- Hover information
  - Shows what references resolved to
- Go-to Reference (`Ctrl-Click`)
- Document Links
  - Used when referencing a file directly in FPP (for example the `instance` `at` specifier).

## Technical Description

This VSCode extension is essentially a FPP compiler frontend
written in TypeScript using ANTLR4. It injests a 'locs' file
generated during the FPrime build process which will tell the
compiler which files to include during its variable/type declaration
stage.

Files are parsed and reduced in a separate worker thread and then
sent through the compilers declaration collection in the main thread.

## Development instructions

To set up dependencies you will need NodeJS and a package manager like `npm` or `yarn`:

```
$ yarn install
```

When making a change to the ANTLR definition (`src/grammar/Fpp.g4`), you will need to regenerate
the generated files.

```
$ yarn antlr
```

To build the extension into bundled JavaScript:

```
$ yarn build
```

Once the extension is built, there should be an `out/` directory at the project root.
You can then test the extension by clicking "Run and Debug" and then clicking the run button on "Run Extension"
to launch an development VSCode environment.

To package the extension into a VSIX file you can use:
```
$ yarn package
```

This will generate a `.vsix` file, from which an extension can be installed in VSCode following instructions: [Install from a VSIX](https://code.visualstudio.com/docs/editor/extension-marketplace#_install-from-a-vsix)

To clean the build artifacts, you can use:
```
$ yarn clean
```
