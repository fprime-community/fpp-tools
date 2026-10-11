# The template enters a constant and then a type, so the value name group is
# merged into the global scope before the type name group and the first
# redefinition reported is a
constant a = 0
type b = U8
module template T() {
  constant a = 1
  type b = U16
}
expand T()
