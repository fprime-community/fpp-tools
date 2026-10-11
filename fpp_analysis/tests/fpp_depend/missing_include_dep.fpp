# Rust-only: a dependency file that includes a file that cannot be opened is a
# missing dependency, and its own dependencies are not followed
locate constant a at "missing_include_dep_dep.fpp"

constant x = a
