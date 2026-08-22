module M {
    state machine S {
        type T
        type Alias = U32
        array Arr = [3] U32
        constant c = 1
        enum E {
            A
            B
        }
        struct St {
            x: U32
        }
        signal sig: E
        action act: Arr
        guard g
        initial enter Idle
        state Idle {
            on sig enter Run
        }
        state Run {
            entry do { act }
            exit do { act }
        }
    }
}
