# The expansion of B comes first in the model, and the body of a template is
# checked where it is expanded, so the nested template in B is the first error

expand B()

module template A() {
  module template A1() {}
}

module template B() {
  module template B1() {}
}
