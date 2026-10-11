module Fw {
  port DpGet
  port DpSend
  port Time
}

passive component C {

  product get port productGetOut

  product send port productSendOut

  time get port timeGetOut

  product record R1: U32 id 9

  product record R2: U32 id 2

  product record R3: U32 id 5

}
