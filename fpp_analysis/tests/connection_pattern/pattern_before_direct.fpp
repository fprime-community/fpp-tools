port P
passive component C {
  sync input port pIn: P
  output port pOut: P
}
instance c: C base id 0x100
topology T {
  instance c
  health connections instance c
  health connections instance c
  connections X {
    c.pIn -> c.pOut
  }
}
