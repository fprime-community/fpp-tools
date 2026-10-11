array B = [2] U32
module template T(type U) {
  array Y = [2] U
  constant c = sizeof(Y)
  array Z = [c] U8 default [1, 2]
}
expand T(type B)
