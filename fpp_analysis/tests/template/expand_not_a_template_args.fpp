# Supplying arguments does not make the name in an expansion specifier resolve
# to a template.

module N {
  constant x = 1
}

module M {
  expand N(constant 1)
}
