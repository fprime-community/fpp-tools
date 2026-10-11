port P
passive component C1 {
  output port p1Out: [2] P
}
passive component C2 {
  sync input port p1In: [2] P
}
instance c1: C1 base id 0x100
instance c2: C2 base id 0x200
topology T {
  instance c1
  instance c2
  connections C {
    c1.p1Out["a"] -> c2.p1In
  }
}
