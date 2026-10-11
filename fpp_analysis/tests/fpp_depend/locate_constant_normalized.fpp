# Rust-only: two location paths that normalize to the same file are consistent
locate constant c at "locate_constant_normalized_a.fpp"
locate constant c at "./locate_constant_normalized_a.fpp"

constant x = c
