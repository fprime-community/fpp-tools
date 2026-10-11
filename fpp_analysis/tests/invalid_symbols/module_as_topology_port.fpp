module M {
  port P
  passive component C {
    sync input port q: P
  }
  instance c: C base id 0x100
  topology T {
    instance c
    port x = M.q
  }
}
