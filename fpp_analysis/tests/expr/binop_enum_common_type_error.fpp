# The sum of two enumerated constants has type Integer (spec, Type Checking,
# Arithmetic and Concatenation), so it has no common type with a string
enum E { X = 1, Y = 2 }
constant c = E.X + E.Y
constant d = [c, "a"]
