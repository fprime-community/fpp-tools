type FwSizeStoreType = U16
constant FW_FIXED_LENGTH_STRING_SIZE = 256

array A = [3] U32
struct S { x: U32, y: U8 }
enum E { X, Y }
constant K = 12

type T1 = string size sizeof(A)
type T2 = string size sizeof(S)
type T3 = string size sizeof(E)
type T4 = string size K
type T5 = string size sizeof(string size sizeof(A))

array A2 = [2] string size sizeof(A)
struct S2 { m: string size sizeof(S) }
