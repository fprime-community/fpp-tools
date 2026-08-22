@ Component instances, init specifiers, and locate specifiers
module Deployment {
    @ Minimal instance
    instance simpleInst: Comp base id 0x100

    @ Instance with every optional clause
    instance fullInst: NS.Comp base id 0x200 \
        type "SpecialType" \
        at "path/to/file.fpp" \
        queue size 10 \
        stack size 4096 \
        priority 5 \
        cpu 1 {
            phase Fpp.ToCpp.Phases.configObjects "code goes here"
            phase Fpp.ToCpp.Phases.instances "more code"
        }

    @ Instance with just queue and stack
    instance workerInst: Worker base id 0x300 queue size 20 stack size 8192

    @ Locate specifiers of each kind
    locate component Foo at "components/Foo.fpp"
    locate constant BAR at "constants/Bar.fpp"
    locate instance baz at "instances/Baz.fpp"
    locate port MyPort at "ports/MyPort.fpp"
    locate type MyType at "types/MyType.fpp"
    locate interface MyInterface at "interfaces/MyInterface.fpp"
    locate state machine MySM at "sm/MySM.fpp"
    locate system MySystem at "systems/MySystem.fpp"

    @ System definition
    system MySystem: NS.MyTopology
}
