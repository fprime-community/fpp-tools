# An absolute template name refers to the template in the global scope, so the
# expansion does not define d, which the template of the enclosing module defines
module template T { constant c = 0 }
module M {
  module template T { constant d = 0 }
  expand .T
  constant x = d
}
