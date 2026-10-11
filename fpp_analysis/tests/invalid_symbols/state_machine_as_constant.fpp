# A qualified name that names a state machine is not a constant
module M { state machine S { initial enter A
  state A } }
constant c = M.S
