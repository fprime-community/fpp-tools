# Rust-only: an include specifier in a state machine is resolved
locate type T at "include_in_state_machine_t.fpp"

state machine SM {
  include "include_in_state_machine.fppi"
  initial enter S
  state S
}
