# The name in an expansion specifier resolves to a module definition, which is
# not a template.

module N {
  constant x = 1
}

module M {
  expand N
}
