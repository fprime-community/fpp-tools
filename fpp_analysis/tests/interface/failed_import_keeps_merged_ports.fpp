port P()

interface A {
    output port x: P
}

interface B {
    output port x: P
}

interface Mid {
    import A
    import B
}

interface Outer {
    import Mid
}

passive component C {
    import Outer
}

passive component D {
    sync input port i: P
}

instance c: C base id 0x100
instance d: D base id 0x200

topology T {
  instance c
  instance d

  connections X {
    c.x -> d.i
  }
}
