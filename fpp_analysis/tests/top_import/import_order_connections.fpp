port P
passive component C {
  sync input port pIn: P
  output port pOut: P
}
instance w: C base id 0x100
instance x: C base id 0x200
instance y: C base id 0x300
instance z: C base id 0x400
topology A {
  import X
  import W
  import Y
  import Z
}
topology W {
  instance w
  connections CW {
    w.pIn -> w.pIn
  }
}
topology X {
  instance x
  connections CX {
    x.pIn -> x.pIn
  }
}
topology Y {
  instance y
  connections CY {
    y.pIn -> y.pIn
  }
}
topology Z {
  instance z
  connections CZ {
    z.pIn -> z.pIn
  }
}
