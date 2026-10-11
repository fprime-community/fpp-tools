# Opcodes are kept as 128-bit integers. The default opcode after the largest
# one is out of that range, so it is reported as an error where Scala, with
# arbitrary-precision opcodes, assigns it.

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
