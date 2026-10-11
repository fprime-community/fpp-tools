interface I {}
module M {}
module template T(instance i: I) {
  constant x = 0
}
expand T(instance M)
