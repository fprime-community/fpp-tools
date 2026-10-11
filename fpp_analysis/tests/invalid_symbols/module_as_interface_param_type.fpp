module M {}
passive component C {}
instance c: C base id 0x100
module template T(instance i: M) {
  constant k = 0
}
expand T(instance c)
