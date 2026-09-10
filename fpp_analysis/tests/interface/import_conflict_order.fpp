port P()

interface A {
    output port p: P
}

interface B {
    output port p: P
}

interface D {
    output port p: P
    import A
    import B
}
