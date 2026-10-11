# A connection port number must have a numeric type
port P
passive component C {
  output port pOut: [2] P
  sync input port pIn: [2] P
}
instance c: C base id 0x100
topology T {
  instance c
  connections C1 { c.pOut["a"] -> c.pIn[0] }
}
