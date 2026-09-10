@ A failed constant is reported once, not once per use. Evaluation stops at the
@ first failure, so the chain of uses below cannot re-report it: each doubling
@ used to double the diagnostic count.
constant z = 1 / 0
constant a = z + z
constant b = a + a
constant c = b + b
constant d = c + c
