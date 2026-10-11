# Rust-only: a dotted constant use is looked up under its name in source order
locate constant M.N.a at "expr_dot_name_order_right.fpp"
locate constant M.a.N at "expr_dot_name_order_wrong.fpp"

constant b = M.N.a
