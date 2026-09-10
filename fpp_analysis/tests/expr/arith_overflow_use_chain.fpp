@ The arithmetic-overflow counterpart of `div_by_zero_use_chain`.
constant max = 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF
constant z = max + 1
constant a = z + z
constant b = a + a
constant c = b + b
constant d = c + c
