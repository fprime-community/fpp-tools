# Rust-only: a dictionary definition in a state machine is not checked as a
# dictionary definition, so a dictionary constant of array type is accepted there
state machine S {
  dictionary constant c = [1, 2]
  initial enter A
  state A
}
