# A shift operand may not have an alias type
type T = U32
module template M(constant p: T) {
  constant c = p << 1
}
expand M(constant 3)
