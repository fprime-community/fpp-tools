port P

passive component C1 {

  output port pOut: [2] P
  sync input port pIn: [2] P

  match pOut with pIn

}

passive component C2 {

  output port pOut: [2] P
  sync input port pIn: [2] P

}

passive component C3 {

  output port pOut: P
  sync input port pIn: P

}

instance c1: C1 base id 0x100
instance c2: C2 base id 0x200
instance c3: C3 base id 0x300

topology A {

  instance c1

  port aOut = c1.pOut
  port aIn = c1.pIn

}

topology T {

  instance A
  instance c2
  instance c3

  connections P {

    unmatched A.aOut[0] -> c3.pIn
    unmatched c3.pOut -> A.aIn[1]

    A.aOut -> c2.pIn
    c2.pOut -> A.aIn

  }

}
