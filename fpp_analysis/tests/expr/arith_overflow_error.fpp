@ The largest value an i128 can hold
constant max = 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF
@ The smallest value an i128 can hold
constant min = (0 - 0x40000000000000000000000000000000) * 2
constant addOverflow = max + 1
constant subOverflow = min - 1
constant mulOverflow = max * 2
constant divOverflow = min / (0 - 1)
constant negOverflow = -min
@ Overflow in a typed context: the element type does not shrink the operands,
@ which are evaluated before conversion
array A = [1] U32 default [max + 1]
