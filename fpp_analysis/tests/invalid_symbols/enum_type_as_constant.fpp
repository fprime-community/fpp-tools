# A qualified name that names an enum type is not a constant
module M { enum E { X, Y } }
constant c = M.E
