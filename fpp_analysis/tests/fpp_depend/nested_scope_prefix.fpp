# Rust-only: a use inside a component inside a module is looked up in the
# enclosing module before the component
locate port Ref.P at "nested_scope_prefix_ref_p.fpp"
locate port C.P at "nested_scope_prefix_c_p.fpp"

module Ref {
  passive component C {
    sync input port pIn: P
  }
}
