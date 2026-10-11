state machine M {
  guard g
  signal s1
  signal s2
  initial enter S
  state S {
    on s1 enter C1
    on s2 enter C3
  }
  choice C1 { if g enter S else enter C2 }
  choice C2 { if g enter S else enter C1 }
  choice C3 { if g enter S else enter C4 }
  choice C4 { if g enter S else enter C3 }
}
