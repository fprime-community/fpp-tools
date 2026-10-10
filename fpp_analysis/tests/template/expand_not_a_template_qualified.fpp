# A qualified name in an expansion specifier resolves to a nested module, which
# is not a template.

module A {
  module B {
    constant x = 1
  }
}

module M {
  expand A.B
}
