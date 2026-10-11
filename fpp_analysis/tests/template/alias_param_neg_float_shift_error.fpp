# Negating a value of an alias of a floating-point type yields type Integer
# but a floating-point value, which a shift cannot take
type F = F32
module template M(constant p: F) {
  constant c = (-p) << 1
}
expand M(constant 1.5)
