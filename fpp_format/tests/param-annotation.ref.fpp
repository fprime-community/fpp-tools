module Fw {
    @ Port for sending a buffer
    port BufferSend(
        @ The buffer
        ref fwBuffer: Fw.Buffer
    )

    @ Port for getting a buffer
    port BufferGet(
        @ The requested size
        $size: FwSizeType
    ) -> Fw.Buffer

    @ A port whose params carry no annotations still flattens
    port Plain(a: U32, b: U32)
}
