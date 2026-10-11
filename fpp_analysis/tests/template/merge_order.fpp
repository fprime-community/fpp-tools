# The template enters c, b and a, in that order, into the scope that is merged
# into the global scope, so the first redefinition reported is c
constant a = 0
constant b = 0
constant c = 0
module template T() {
  constant c = 3
  constant b = 2
  constant a = 1
}
expand T()
