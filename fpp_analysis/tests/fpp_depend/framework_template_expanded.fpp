# Rust-only: a component in an expanded template has framework dependencies
port P
module template T(constant p: U32) {
  passive component C {}
  active component D { guarded input port pIn: P }
}
module M {
  expand T(constant 1)
}
