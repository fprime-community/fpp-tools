port P
passive component C {
  output port pOut: [10] P
  sync input port pIn: [10] P
  match pOut with pIn
}
passive component D {
  sync input port pIn: P
  output port pOut: P
}
instance c: C base id 0x100
instance d0: D base id 0x200
instance d1: D base id 0x300
instance d2: D base id 0x400
instance d3: D base id 0x500
topology T {
  instance c
  instance d0
  instance d1
  instance d2
  instance d3
  connections X {
    c.pOut -> d0.pIn
    c.pOut -> d1.pIn
    c.pOut -> d2.pIn
    c.pOut -> d3.pIn
  }
}
