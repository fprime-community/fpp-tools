port P
passive component C {
  sync input port pIn: P
  output port pOut: P
}
instance c: C base id 0x100
module template Tm(constant k: U32) {
  topology A {
    instance c
    connections X {
      c.pIn -> c.pOut
    }
  }
}
topology B {
  instance c
  connections Y {
    c.pIn -> c.pIn
  }
}
expand Tm(constant 0)
