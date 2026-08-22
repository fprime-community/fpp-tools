@ State machine test
module StateMachines {
    @ Simple state machine
    state machine SimpleSM {
        @ Signal definitions
        signal tick
        signal dataReady: U32
        signal timeout

        @ Action definitions
        action processData
        action logError: U32
        action reset

        @ Guard definitions
        guard isReady
        guard hasData: U32

        @ Initial transition
        initial enter Idle

        @ States
        state Idle

        state Active {
            entry do { processData }
            exit do {
                reset
                logError
            }

            @ Nested states
            state Processing

            @ Initial transition for nested state
            initial do { processData } enter Processing

            @ Transitions
            on tick enter Idle
            on dataReady if isReady do { processData } enter Processing
            on timeout if hasData enter Idle
        }

        @ State with choice
        state Waiting {
            @ Choice definition
            choice CheckCondition {
                if isReady enter Active else enter Idle @< choice transition annotation
            }

            @ Transition to choice
            on tick enter CheckCondition
            on dataReady do {
            processData
            }
        }
    }

    @ Complex state machine
    state machine ComplexSM {
        signal s1
        signal s2: U32
        signal s3

        action a1
        action a2: F32
        action a3

        guard g1
        guard g2: bool

        initial do {
            a1
            a2
        } enter S1

        choice C1 {
            if g1 do { a1 } enter S2 else do { a2 } enter S3
        }

        choice C2 {
            if g2 do {
                a1
                a2
                a3
            } enter S2 else enter S3
        }

        choice C3 {
            if g2 do {
            a1a2a3
            } enter S2 else enter S3
        }

        choice RESUME {
                if pendingHostFunction do { dispatchPendingHostFunction } enter AWAITING_RESPONSE \
                    else enter SPINNING
                }

        state S1 {
            on s1 if g1 do {
            a1
            } enter C1
            on s2 enter S2
        }

        state S2

        state S3 {
            initial enter S2
        }
    }
}
