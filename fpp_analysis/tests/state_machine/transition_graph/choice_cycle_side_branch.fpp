state machine M {
  guard g
  signal s
  initial enter S
  state S {
    on s enter A
  }
  choice A { if g enter B else enter S }
  choice B { if g enter Y else enter A }
  choice Y { if g enter S else enter S }
}
