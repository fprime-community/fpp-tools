# The expansion of B comes first in the model, but the template definitions are
# checked in source order before any template is expanded, so the nested
# template in A is the first error

expand B()

module template A() {
  module template A1() {}
}

module template B() {
  module template B1() {}
}
