port P()

interface Imported {
    output port a1: [1] P
    output port a2: [2] P
}

interface I {
    import Imported
    output port b1: [3] P
    output port b2: [4] P
}

passive component C1 {

  output port q: [9] P

}

instance c1: C1 base id 0x100

topology A implements I {
  instance c1

  port other = c1.q
}
