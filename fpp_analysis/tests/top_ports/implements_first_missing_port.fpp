port P

interface I {
    output port sss: [1] P
    output port rrr: [2] P
    output port qqq: [3] P
}

passive component C1 {

  output port q: [9] P

}

instance c1: C1 base id 0x100

topology A implements I {
  instance c1

  port other = c1.q
}
