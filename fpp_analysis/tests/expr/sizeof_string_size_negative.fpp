# Rust-only: the size of a string type name must lie in [0, 2^31) (spec,
# String Type Names), also where the type name is the operand of a sizeof
type FwSizeStoreType = U16
constant FW_FIXED_LENGTH_STRING_SIZE = 256
constant c = sizeof(string size -1)
array B = [c - 1] U8
