port P
passive component C {
  output port pOut: P
}
instance c1: C base id 0x100
instance c2: C base id 0x200
instance c3: C base id 0x300
topology T {
  instance c1
  port d = c2.pOut
  port a = c3.pOut
}
