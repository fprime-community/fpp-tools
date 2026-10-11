state machine M {
  guard g
  initial enter S
  choice C { if g enter S else enter S }
  state S {
    on s enter C.X
  }
  signal s
}
