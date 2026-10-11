module M {
  port P
  passive component C {
    output port p: P
    sync input port q: P
  }
  instance c: C base id 0x100
  topology T {
    instance c
    connections X {
      M.p -> c.q
    }
  }
}
