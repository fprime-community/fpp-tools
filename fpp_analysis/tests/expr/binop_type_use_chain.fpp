@ The counterpart of `div_by_zero_use_chain` for a failure raised while typing
@ the expression rather than while evaluating it.
constant z = 1 + true
constant a = z + z
constant b = a + a
constant c = b + b
constant d = c + c
