# Rust-only: a dictionary constant in a template body whose type is a
# template parameter of alias type is rejected
type A = U32
module template T(constant p: A) {
  dictionary constant c = p
}
module M { expand T(constant 10) }
