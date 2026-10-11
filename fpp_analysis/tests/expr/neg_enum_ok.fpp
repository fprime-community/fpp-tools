# Negating an enum constant yields type Integer
enum E { A, B }
constant c = -E.B
array X = [3] I32 default c
