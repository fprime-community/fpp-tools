port P1
passive component C1 {
  output port p1Out: [2] P1
  sync input port p1In: [2] P1
}
instance c1: C1 base id 0x100
instance c2: C1 base id 0x200
topology T {
  instance c1
  instance c2
  connections A {
    c1.p1Out[0x100000000] -> c2.p1In
  }
}
