array B = [2] U32
struct T { b: B }
array A = [1] T default [{ b = [1, 2, 3] }]
