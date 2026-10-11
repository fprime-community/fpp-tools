# Selecting a member that a struct type does not have
struct S { x: U32 }
module template M(constant p: S) {
  constant c = p.y
}
expand M(constant { x = 1 })
