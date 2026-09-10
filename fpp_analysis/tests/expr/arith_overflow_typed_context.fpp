@ The largest value an i128 can hold
constant max = 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF
@ Overflow in a typed context: the element type does not shrink the operands,
@ which are evaluated before conversion
array A = [1] U32 default [max + 1]
