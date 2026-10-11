port P
passive component C {
  sync input port pIn: P
  output port pOut: P
}
instance c: C base id 0x100
include "include_order.fppi"
topology A {
  instance c
  connections X {
    B.b -> c.pIn
  }
}
