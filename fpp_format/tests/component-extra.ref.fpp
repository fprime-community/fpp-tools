@ Component member variants missing from the main component fixture
queued component ExtraComp {
    @ Command with queue-full behaviors
    async command Enqueue opcode 0x2 priority 3 drop
    async command Hooked opcode 0x3 hook
    sync command Blocking opcode 0x4 block

    @ Command with interleaved annotations on arguments
    @ If $block == Svc.BlockState.BLOCK this command will wait for completion.
    async command RUN(
        fileName: string size FileNameStringSize  @< The name of the sequence file
        $block: Svc.BlockState                    @< Return command status when complete or not
    ) \
        opcode 0x0

    @ Wait for the interpreter to finish and return it's result as a CmdResponse
    async command WAIT opcode 0x1

    # async command RUN_ARGS(
    #                       fileName: string size FileNameStringSize @< The name of the sequence file
    #                       $block: Svc.BlockState @< Return command status when complete or not
    #                       buffer: Svc.SeqArgs @< Arguments to pass to the sequencer
    #                     ) \
    #     opcode 1 priority 7 assert

    @ Guarded input port
    guarded input port guardedIn: DataPort

    @ Serial port instances
    async input port serialIn: serial
    output port serialOut: serial

    @ Telemetry with update on change
    telemetry changing: U32 id 0x10 update on change

    @ Telemetry with only high limits
    telemetry hot: F32 \
        id 0x11 \
        high {
            red 100.0
        }

    @ Event with warning-low severity
    event Warned() severity warning low id 0x20 format "warned"

    @ Event with diagnostic severity and throttle every
    event Diag(code: U32) \
        severity diagnostic \
        id 0x21 \
        format "diag {}" \
        throttle 5

    @ Event with command severity
    event Cmd() severity command id 0x22 format "cmd"

    @ Text-event special port
    text event port textEventOut

    @ Special ports: command recv/reg/resp, product variants
    command recv port cmdIn
    command reg port cmdRegIn
    command resp port cmdResp
    product get port prodGet
    product recv port prodRecv
    product request port prodReq
    product send port prodSend

    @ Param with save opcode
    param saved: U32 default 0 id 0x30 save opcode 0x31
}
