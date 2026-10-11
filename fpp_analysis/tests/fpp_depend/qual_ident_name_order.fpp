# Rust-only: a qualified type name of four parts is looked up under its name in
# source order
locate type A.B.C.T at "qual_ident_name_order_right.fpp"
locate type A.C.B.T at "qual_ident_name_order_wrong.fpp"

array X = [2] A.B.C.T
