# Negating an enum constant yields type Integer
enum E { A = 1, B = 2 }
struct S { x: E } default { x = -E.A }
