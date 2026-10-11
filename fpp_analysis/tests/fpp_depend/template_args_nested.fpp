# Rust-only: the arguments of an expansion are analyzed at the expansion, in the
# scope that encloses it
locate template D.T at "template_args_nested_d_t.fpp"
locate constant D.x at "template_args_nested_d_x.fpp"
locate constant A.B.y at "template_args_nested_ab_y.fpp"
locate constant A.y at "template_args_nested_a_y.fpp"
locate type A.Ty at "template_args_nested_a_ty.fpp"

module D {
  module template T(constant p: U32, type U) {
    constant z = x
    struct S { m: U }
  }
}

module A {
  module B {
    expand D.T(constant y, type Ty)
  }
}
