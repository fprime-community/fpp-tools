array B = [2] U32
module template T(type U) {
  array Y = [2] U
  constant c = sizeof(Y)
}
expand T(type B)
constant FW_ASSERT_COUNT_MAX = c - 100
