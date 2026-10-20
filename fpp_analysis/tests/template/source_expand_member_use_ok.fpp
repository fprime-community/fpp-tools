# The definitions of an expansion written in the source are definitions of the
# enclosing module, and the uses inside it resolve against them and against the
# template parameters

module template T(constant p: U32) {

}

module M {

  expand T(constant 1) {

    constant a = p

    constant b = a + 1

    state machine S {
      initial enter S1
      state S1
    }

  }

}

constant c = M.a + M.b

constant d = M.S.State.S1
