module Fw {
    port Cmd
    port CmdReg
}

interface A {
    command recv port ar
    command reg port ag
}

interface D {
    command recv port dr
    command reg port dg
    import A
}
