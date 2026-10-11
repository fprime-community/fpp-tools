# Rust-only: the body of a template is analyzed at its definition and, after
# the expansion, at the expansion
locate constant M.x at "body_at_expansion_site_m_x.fpp"
locate constant x at "body_at_expansion_site_x.fpp"

module template T(constant p: U32) {
  constant y = x
}

module M {
  expand T(constant 1)
}
