port P

passive component C1 {

  output port pOut1: [2] P

  output port pOut2: [2] P

  match pOut1 with pOut2

}

passive component C2 {

  sync input port pIn: P

}

instance c1: C1 base id 0x100
instance c2: C2 base id 0x200

topology A {

  instance c1
  instance c2

  port a = c1.pOut1
  port b = c2.pIn

}

topology B {

  instance A

  connections C {

    unmatched A.a -> A.b

  }

}
