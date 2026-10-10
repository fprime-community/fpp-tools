port P

interface I {

  sync input port pIn: P

}

passive component C {

  sync input port pIn: P

}

passive component Hub {

  output port hOut: P

}

instance c1: C base id 0x100
instance hub: Hub base id 0x300

module template T(instance i: I) {

  topology Sub {

    instance i
    instance hub

    connections C1 {
      hub.hOut -> i.pIn
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
