port P
port Q

passive component C {
  output port pOut: P
  sync input port pIn: P
  sync input port qIn: Q
}

instance c1: C base id 0x100
instance c2: C base id 0x200

topology T {
  instance c1
  instance c2
  connections A {
    unmatched c1.pOut -> c2.pIn
    c2.pOut -> c1.qIn
  }
}
