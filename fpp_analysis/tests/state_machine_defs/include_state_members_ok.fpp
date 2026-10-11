# Members included into a state are members of that state
state machine M {
  signal s
  initial enter S
  state S {
    initial enter T
    include "include_state_members.fppi"
  }
}
