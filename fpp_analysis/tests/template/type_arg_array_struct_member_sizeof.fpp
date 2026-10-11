array B = [2] U32
module template T(type U) {
  struct W { u: U }
  constant c = sizeof(W)
  array Z = [c - 1000] U8
}
expand T(type B)
