# Arithmetic on a value of an alias type has the numeric type that the alias
# converts to
type A = U32
module template T(constant p: A) {
  constant d = p + p
  constant e = p * p
  constant f = (-p) << 1
  constant g = [p, p]
}
expand T(constant 1)
