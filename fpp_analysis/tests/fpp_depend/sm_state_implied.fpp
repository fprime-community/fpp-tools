# Rust-only: a state machine implies a use of its State enum, under the
# qualified name of the state machine
module M {
  locate type SM.State at "sm_state_implied_state.fpp"
  state machine SM {
    initial enter S
    state S
  }
}
