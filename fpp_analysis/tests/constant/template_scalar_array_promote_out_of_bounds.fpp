array B = [2] U32

module template T(constant b: B) {
  constant inner = [ 1, b ]
  constant outer = [ inner, inner ]
  constant x = outer[0][0][2]
}

module M {
  expand T(constant [2, 3])
}
