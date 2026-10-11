# The string size of a constant parameter type is checked when the bound
# argument is evaluated, before later constants
type FwSizeStoreType = U16
constant FW_FIXED_LENGTH_STRING_SIZE = 80
module template M(constant s: string size -1) {
  constant c = s
}
expand M(constant "abc")
constant z = 1 / 0
