# Rust-only: an input file that includes a file that cannot be opened is an
# error
include "missing_include_input_nope.fppi"
locate constant a at "missing_include_input_a.fpp"

constant x = a
