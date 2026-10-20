module template T(constant p: U32) {
  constant c = p
}

module M {
    @ An expanded template
  expand T(constant 1)   {   constant c = 1
      @ A nested module
      module N {
  constant d = c }

  }

  expand T(constant 2) {}

  expand T(
    constant 3
  ) {
    constant c = 3
  }
}
