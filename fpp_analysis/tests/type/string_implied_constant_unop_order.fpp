# A default-size string type checks the expression of FW_FIXED_LENGTH_STRING_SIZE
# at the point of the string type
type FwSizeStoreType = U16
array A = [3] string
constant x = -true
constant FW_FIXED_LENGTH_STRING_SIZE = -false
