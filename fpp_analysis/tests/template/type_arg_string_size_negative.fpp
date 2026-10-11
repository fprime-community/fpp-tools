type FwSizeStoreType = U16
constant FW_FIXED_LENGTH_STRING_SIZE = 256
module template T(type U) {
  constant c = 0
}
expand T(type string size -1)
