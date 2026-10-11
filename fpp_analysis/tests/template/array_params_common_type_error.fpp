array A = [3] U32
array B = [3] U32
module template T(constant p: A, constant q: B) {
  constant c = p + q
}
expand T(constant 1, constant 2)
