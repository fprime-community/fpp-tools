# Re-export the compiled extension (`fpp.fpp`)

from .fpp import *

__doc__ = fpp.__doc__
if hasattr(fpp, "__all__"):
    __all__ = fpp.__all__
