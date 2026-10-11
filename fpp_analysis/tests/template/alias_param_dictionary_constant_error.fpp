# A dictionary constant may not have an alias type
type A = U32
module template T(constant c: A) {
    dictionary constant k = c
}
module M {
    expand T(constant 1)
}
