  # constant =     1

constant f = 1

+ c

+ c

constant g = 2

active component C {
    telemetry port pOut

    async input i: d -> d

    event i (

    ) severity warning low format "hello" throttle 1

