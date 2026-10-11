port P
passive component C {
  sync input port pIn: P
  output port pOut: P
}
instance c: C base id 0x100
topology A {
  instance c
  port a = B.b
}
topology B {
  instance c
  port b = c.pOut
}
