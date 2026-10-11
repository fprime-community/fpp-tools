# Each definition below has one syntax error. Where it comes before the commit
# point of the failing member, the error is the closing brace expected at the
# start of the member (or after the shorter alternative of an alias type).

state machine S1 {
  initial enter 3
}

state machine S2 {
  state A {
    entry foo
  }
}

passive component C1 {
  param p U32
}

passive component C2 {
  telemetry 3: U32
}

passive component C3 {
  command foo port p
}

passive component C4 {
  product container C default 3
}

module M1 {
  type T = 1
}

module M2 {
  constant a = 1 constant b = 2
}

module M3 {
  state machine 3
}
