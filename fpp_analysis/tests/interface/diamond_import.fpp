port P()

interface A {
    output port pOut: P
}

interface B {
    import A
}

interface C {
    import A
}

interface D {
    import B
    import C
}
