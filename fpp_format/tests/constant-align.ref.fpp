module Constants {
    constant a    = 1
    constant bbbb = 2
    constant cc   = 0x10

    @ A documented constant breaks the alignment run
    constant dddddd = 4
    constant e      = 5

    enum E {
        X   = 1
        YYY = 22
        Z   = 333
    }
}
