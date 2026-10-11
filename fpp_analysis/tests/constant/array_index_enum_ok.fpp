# An enum constant index is converted to Integer, so c = a[1] = 2
enum E { X, Y }
constant a = [1, 2, 3]
constant c = a[E.Y]
array Z = [c] U8 default [1, 2]
