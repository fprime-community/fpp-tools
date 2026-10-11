# The interface has more than four ports. A port interface keeps its ports in
# the order they were added, so the first missing port reported is the first one
# the interface declares (p5), however many ports the interface has, and not
# the first in the order of the port names or sizes.

port P()

interface I {
    output port p5: [5] P
    output port p4: [4] P
    output port p3: [3] P
    output port p2: [2] P
    output port p1: [1] P
}

passive component C1 {

  output port q: [9] P

}

instance c1: C1 base id 0x100

topology A implements I {
  instance c1

  port other = c1.q
}
