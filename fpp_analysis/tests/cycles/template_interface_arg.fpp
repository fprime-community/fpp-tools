interface I { }
module template T(instance i: I) {
  topology X { instance i }
}
topology A { instance X }
expand T(instance A)
