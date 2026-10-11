constant y = cOut
module template T(constant p: U32) {
    constant cOut = p + 2
}

expand T(constant y)
