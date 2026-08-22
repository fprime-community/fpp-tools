@ Component for testing
active component TestComponent {
    @ Command 1
    async command StartTest(testId: U32, timeout: F32) \
        opcode 0x10 \
        priority 5 \
        assert

    @ Command 2
    sync command StopTest

    @ Telemetry channels
    telemetry temperature: F32 \
        id 0x01 \
        update always \
        format "{} C" \
        low {
            yellow 10.0
            orange 5.0
            red 0.0
        } \
        high {
            yellow 50.0
            orange 60.0
            red 70.0
        }

    telemetry statusCode: U32 id 0x02

    @ Events
    event TestStarted(testId: U32) \
        severity activity high \
        id 0x100 \
        format "Test {} started" \
        throttle 10

    event TestFailed() severity fatal format "Test failed"

    @ A COMMAND event emitted by the guest program
    event LogCommand(msg: string size 128) severity command format "{}"

    @ A COMMAND event emitted by the guest program
    event LogCommand(msg: string size 128) severity command format "{}"

    event ManyArguments(
        r0: U8
        r1: U8
        r2: U8
        r3: U8
        r4: U8
        r5: U8
        r6: U8
        r7: U8
        r8: U8
        r9: U8
        rA: U8
        rB: U8
        rC: U8
        rD: U8
        rE: U8
        rF: U8
    ) \
        severity activity low \
        format "{x} {x} {x} {x} {x} {x} {x} {x} {x} {x} {x} {x} {x} {x} {x} {x}"

    @ Parameters
    param maxIterations: U32 \
        default 100 \
        id 0x20 \
        set opcode 0x21 \
        save opcode 0x22

    external param configFile: string id 0x30

    @ Port instances
    async input port dataIn: [10] DataPort priority 10 drop
    output port dataOut: [5] DataPort

    sync input port controlIn: ControlPort priority 5 assert

    command recv port cmdRecv
    command reg port cmdReg
    telemetry port tlmOut
    event port eventOut
    time get port timeGet
    param get port prmGet
    param set port prmSet

    @ Internal ports
    internal port ProcessData(data: U32, sz: U32) priority 10 block

    @ Port matching
    match dataIn with dataOut

    @ Container and record
    product container TestContainer id 0x50 default priority 5
    product record TestRecord: U32 array id 0x60

    @ State machine
    state machine TestSM
    state machine instance smInst: TestSM priority 10 hook

    @ Type definitions inside component
    type LocalType = U32
    array LocalArray = [3] F32 default 0.0
    struct LocalStruct {
        x: U32
        y: F32
        z: string
    }
    enum LocalEnum {
        IDLE
        RUNNING
        STOPPED
    } default IDLE

    @ Constant definition
    constant LOCAL_CONST = 42
}
