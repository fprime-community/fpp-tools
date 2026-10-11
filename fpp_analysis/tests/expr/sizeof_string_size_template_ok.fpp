# The operand of a sizeof may be a type parameter bound to a
# string type name, whose size the argument gives: c is 2 + 4, and A has
# size 1
type FwSizeStoreType = U16
constant FW_FIXED_LENGTH_STRING_SIZE = 256
module template M(type T) {
  constant c = sizeof(T)
  array A = [7 - c] U8
}
expand M(type string size 4)
