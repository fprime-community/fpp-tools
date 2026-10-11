# A floating-point index is converted to Integer, so c = a[1] = 2
constant a = [1, 2, 3]
constant c = a[1.5]
array Z = [c] U8 default [1, 2]
