@ A failed constant is reported once, at its definition. Evaluation stops at
@ the first failure, so the chain of uses below reports nothing more, however
@ many times each constant is used.
constant z = 1 / 0
constant a = z + z
constant b = a + a
constant c = b + b
constant d = c + c
