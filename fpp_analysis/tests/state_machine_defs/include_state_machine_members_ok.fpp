# Members included into a state machine are members of that state machine
state machine M {
  signal s
  initial enter T
  include "include_state_machine_members.fppi"
}
