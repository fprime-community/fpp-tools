# An arithmetic expression whose common type is an alias of an
# integer type has type Integer (spec, Type Checking, Arithmetic and
# Concatenation: a common type that is not numeric but may be converted to
# Integer gives Integer), so it may be shifted
type A = U32
module template T(constant q: A) {
  constant d = (q + 1) << 1
  constant e = (q * q) >> 2
  constant f = (-q) >> 2
}
expand T(constant 3)
