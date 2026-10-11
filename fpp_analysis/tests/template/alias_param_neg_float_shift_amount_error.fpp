# Negating a value of an alias of a floating-point type yields type Integer
# but a floating-point value, which a shift cannot take as its amount
type T = F64
module template M(constant f: T) {
  constant c = 1 << -f
}
expand M(constant 1.5)
