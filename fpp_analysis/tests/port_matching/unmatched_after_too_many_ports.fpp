port P
passive component C {
  output port pOut: P
  output port qOut: P
  sync input port pIn: P
  sync input port qIn: P
}
instance c1: C base id 0x100
instance c2: C base id 0x200
instance c3: C base id 0x300
topology T {
  instance c1
  instance c2
  instance c3
  connections X {
    unmatched c1.qOut -> c2.qIn
    c1.pOut -> c2.pIn
    c1.pOut -> c3.pIn
  }
}
