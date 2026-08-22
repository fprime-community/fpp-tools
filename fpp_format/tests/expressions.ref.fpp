@ Expression forms not covered by simple binary constants
module Expressions {
    @ Unary negation
    constant neg = -42

    @ Negation of a parenthesized sum
    constant negSum = -(1 + 2)

    @ Parenthesized precedence
    constant grouped = (1 + 2) * (3 - 4)

    @ Nested parens
    constant nested = ((1))

    @ Array literal expression
    constant arr = [1, 2, 3]

    @ Struct-value expression
    constant pt = { x = 1, y = 2 }

    @ Nested struct value
    constant nestedStruct = { inner = { a = 1 }, flag = true }

    @ Member access on a qualified identifier
    constant member = A.B.value

    @ Subscript expression
    constant elem = data[2]

    @ Chained member and subscript
    constant chained = a.b[0].c

    @ Float and boolean literals
    constant pi   = 3.14
    constant flag = true
    constant off  = false

    @ String literal constant
    constant greeting = "hello"
}
