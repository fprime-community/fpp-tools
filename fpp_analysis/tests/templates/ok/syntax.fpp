module template T() {
    include "../../../../fpp_parser/tests/syntax.fpp"
}

module M1 {
    expand T()
}

module M2 {
    expand T()
}
