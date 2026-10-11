# An array type that is the operand of a sizeof is finalized once, so its
# format error is reported once
array A = [3] U32 format "{f}"
constant c = sizeof(A)
