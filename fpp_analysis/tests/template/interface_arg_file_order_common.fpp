port P
interface I { sync input port pIn: P }
passive component C {}
instance c: C base id 0x100
module template T(instance i: I) {
  constant k = 0
}
