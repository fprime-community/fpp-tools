port P
passive component C {
  sync input port pIn: P
  output port pOut: P
}
instance c: C base id 0x100
topology A {
  instance c
  connections X {
    c.pIn -> c.pIn
  }
}
include "include_first_error.fppi"
