"""Make `import fpp.ast` reach the extension's `ast` submodule."""

import sys

from .fpp import ast as _ast

sys.modules[__name__] = _ast
