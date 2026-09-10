port P()

interface A {
    output port q1: P
    output port q2: P
    output port q3: P
    output port q4: P
}

interface D {
    output port q4: P
    output port q3: P
    output port q2: P
    output port q1: P
    import A
}
