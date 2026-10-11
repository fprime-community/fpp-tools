# Rust-only: the implements clause of a topology is in the scope that encloses
# the topology
locate interface I at "implements_scope_i.fpp"
locate interface T.I at "implements_scope_t_i.fpp"

topology T implements I {}
