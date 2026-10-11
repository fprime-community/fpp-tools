# A shift operand may not have an alias of an enum type
enum E { A = 1, B = 2 }
type T = E
module template M(constant p: T) {
  constant c = p << 1
}
expand M(constant E.A)
