# Rust-only: a dictionary constant whose type is an alias type is rejected,
# even when the alias names a numeric type
type A = F32
module template T(constant p: A) {
  constant c = p
}
module M { expand T(constant 1.0) }
dictionary constant d = M.c
