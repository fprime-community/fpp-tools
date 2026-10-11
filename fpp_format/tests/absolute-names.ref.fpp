module M {
    constant a = 1
    constant b = .a
    constant c = .M.a
    constant d = [.a, -.a, (.a)]
    constant e = .a
    type T = .M.U
    type U = U32
    array A = [.M.a] .M.U default .M.a
    topology T implements .I {
        instance .M.i
        port p = .M.i.out
        connections C {
            .M.i.out -> .M.j.in
        }
        telemetry packets P {
            packet A group 0 {
                .M.i.ch
            }
        } omit {
            .M.i.ch2
        }
    }
    state machine S {
        initial enter .S.A
        state A {
            on s enter .S.A
        }
    }
    expand .M.T(type .M.U, instance .M.i, constant .M.c)
    locate constant .M.c at "c.fpp"
    system Sys: .M.T
}
