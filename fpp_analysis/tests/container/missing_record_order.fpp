module Fw {
  port DpGet
  port DpSend
  port Time
}

passive component C {

  product get port productGetOut

  product send port productSendOut

  time get port timeGetOut

  product container C1 id 9

  product container C2 id 2

  product container C3 id 5

}
