# Adding two enum constants yields type Integer
enum E { A, B }
constant d = E.A + E.B
array X = [2] E default d
