@ Module with port definitions
module Ports {
    @ Simple port without return
    port DataPort(value: U32, timestamp: F32)

    @ Port with return type
    port ComputePort(input1: F32, input2: F32) -> F32

    @ Port without parameters
    port SimpleSignalPort

    @ Port with single parameter
    port NotifyPort(msg: string)

    @ Port with ref parameter
    port RefPort(ref buffer: U32)

    @ Port with multiple params and return
    port ComplexPort(a: U32, b: F32, c: string) -> U32
}
