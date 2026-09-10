port P()

interface Bad {
    output port p: P
    output port p: P
}

interface C {
    import Bad
}
