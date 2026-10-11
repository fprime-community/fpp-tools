passive component C {}
instance c: C base id 0x100
instance d: C base id 0x200
topology A {
  import B
}
topology X {
  instance c
  instance c
}
topology B {
  instance d
  instance d
}
