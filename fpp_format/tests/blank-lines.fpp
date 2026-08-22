# Definition-scope bodies (module, component, interface, topology, state
# machine), individual states, and connection blocks preserve a single blank
# line at their start and end when the source has one. Data-type bodies
# (struct, enum) always hug their delimiters.

module WithBoth {

  constant a = 1

}

module LeadingOnly {

  constant b = 2
}

module TrailingOnly {
  constant c = 3

}

module Nested {

  module Inner {


    constant d = 4


  }

  struct S {

    x: U32

  }

}

module EmptyWithBlank {

}

module EmptyNoBlank {
}

active component Comp {

  sync input port pIn: P

  struct Inner {

    x: U32

  }

}

interface Iface {

  sync input port pIn: P

}

topology Topo {

  connections C {

    a.out -> b.in

  }

}

state machine SM {

  initial enter S

  state S {

    on x enter S

  }

}
