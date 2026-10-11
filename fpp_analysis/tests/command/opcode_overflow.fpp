# Opcodes are kept as 128-bit integers. Command A has the largest one, so the
# default opcode after it, which command B would get, is out of that range:
# the error is reported at A.

module Fw {
  port Cmd
  port CmdReg
  port CmdResponse
}

passive component C {

  command recv port cmdIn

  command reg port cmdRegOut

  command resp port cmdResponseOut

  sync command A opcode 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF

  sync command B

}
