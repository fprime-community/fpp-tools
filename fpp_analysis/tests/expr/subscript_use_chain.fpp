@ The out-of-range-subscript counterpart of `div_by_zero_use_chain`.
array Arr = [2] U8
constant arr = [1, 2]
constant z = arr[5]
constant a = z + z
constant b = a + a
constant c = b + b
constant d = c + c
