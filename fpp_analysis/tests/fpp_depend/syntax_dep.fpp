# Rust-only: a syntax error in a dependency file is an error
locate constant a at "syntax_dep_bad.fpp"
locate constant b at "syntax_dep_good.fpp"

constant x = a + b
