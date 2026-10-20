# The expansion of B comes first in the model, but the template definitions are
# checked in source order before any template is expanded, so the expansion
# specifier in A is the first error

expand B()

module template A() { expand X() }

module template B() { expand Y() }
