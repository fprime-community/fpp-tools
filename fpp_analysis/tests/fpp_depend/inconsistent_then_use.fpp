# Rust-only: an inconsistent location path is an error, so no dependency is
# reported
locate constant c at "inconsistent_then_use_a.fpp"
locate constant c at "inconsistent_then_use_b.fpp"
locate constant d at "inconsistent_then_use_d.fpp"

constant x = d
