port P

interface I {

  output port pOut: [4] P

}

passive component C {

  output port pOut: [4] P

}

passive component Hub {

  sync input port hIn: P

}

instance c1: C base id 0x100
instance hub: Hub base id 0x300

module template T(instance i: I) {

  topology Sub {

    instance i
    instance hub

    connections C1 {
      i.pOut[0] -> hub.hIn
    }

  }

}

module E1 {
  expand T(instance c1)
}

module E2 {
  expand T(instance c1)
}

deployment topology Dep {

  import E1.Sub
  import E2.Sub

}
