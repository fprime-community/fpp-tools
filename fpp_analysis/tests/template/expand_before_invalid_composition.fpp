# The expansion of B comes first in the model, and the body of a template is
# checked where it is expanded, so the expansion specifier in B is the first
# error

expand B()

module template A() { expand X() }

module template B() { expand Y() }
