# fpp-tools

This repository features a variety of tools for the [F Prime Prime](https://github.com/nasa/fpp) (FPP) language.
These tools include:

- Language Server Protocol ([fpp_lsp_server](https://github.com/fprime-community/fpp-tools/tree/master/fpp_lsp_server)): A language server to interface with IDEs. Supported editors include
  - Visual Studio Code
  - Intellij
  - Neovim
  - Emacs
- Formatter ([fpp_format](https://github.com/fprime-community/fpp-tools/tree/master/fpp_format)): A pretty-printer for FPP
- Diagramming ([fpp_diagram](https://github.com/fprime-community/fpp-tools/tree/master/fpp_diagram)): A diagram (text-model) generator for topologies and state machines
- Python Bindings ([fpp_python](https://github.com/fprime-community/fpp-tools/tree/master/fpp_python)): Bindings to Python to interface with the syntax and analysis models
- AST Querying ([fpp-query](https://github.com/fprime-community/fpp-tools/tree/master/fpp_query)): A tool for querying the FPP syntactic model. A general-purpose `fpp-filenames`.

Public facing packages are released as binaries using [PyPI](https://pypi.org/) and may be installed via PIP.
