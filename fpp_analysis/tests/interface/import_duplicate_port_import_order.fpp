port P()

interface Z {
    output port aaa: P
}

interface A {
    import Z
    output port zzz: P
}

interface D {
    output port aaa: P
    output port zzz: P
    import A
}
