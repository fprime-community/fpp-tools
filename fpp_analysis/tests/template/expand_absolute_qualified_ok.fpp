# The qualifier of an absolute template name is resolved in the global scope,
# not in the enclosing module, which has its own module M
module M {
  module template T { constant c = 0 }
}
module N {
  module M { }
  expand .M.T
  constant x = c
}
