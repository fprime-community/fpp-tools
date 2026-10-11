# An absolute template name refers to the template in the global scope, which
# defines c, not to the template of the same name in the enclosing module
module template T { constant c = 0 }
module M {
  module template T { constant d = 0 }
  expand .T
  constant x = c
}
