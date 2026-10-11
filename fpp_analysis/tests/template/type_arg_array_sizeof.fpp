array B = [2] U32
module template T(type U) {
  array Y = [2] U
  constant c = sizeof(Y)
  array Z = [c - 1000] U8
}
expand T(type B)
