array B = [2] U32
array C = [2] B
module template T(constant c: C) {
  array D = [3] U32
  array A = [2] D default c
}
expand T(constant [[1, 2], [3, 4]])
