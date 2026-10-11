# Rust-only: a use two modules deep is looked up in each enclosing module,
# outermost names first
locate constant A.c at "nested_module_prefix_a_c.fpp"
locate constant B.c at "nested_module_prefix_b_c.fpp"

module A {
  module B {
    constant x = c
  }
}
