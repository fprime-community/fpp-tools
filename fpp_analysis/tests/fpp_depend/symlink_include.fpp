# Rust-only: a file that is both included and located is not a dependency, when
# the input path goes through a symbolic link
include "symlink_include.fppi"
locate constant a at "symlink_include.fppi"

constant c = a
