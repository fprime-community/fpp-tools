# The argument bound to a constant parameter whose type is an alias of an
# array type is an array value of the aliased array type
array B = [2] U32
type A = B
module template T(constant p: A) {
  constant c = p[1]
  array X = [c] U8
}
expand T(constant [1, 2])
