# Opcodes are kept as 128-bit integers. The default set opcode of the parameter
# is the largest one, so its default save opcode is out of that range and is
# reported as an error where Scala, with arbitrary-precision opcodes, assigns
# it.

module Fw {
  port Cmd
  port CmdReg
  port CmdResponse
  port PrmGet
  port PrmSet
}

passive component C {

  command recv port cmdIn

  command reg port cmdRegOut

  command resp port cmdResponseOut

  param get port prmGetOut

  param set port prmSetOut

  sync command A opcode 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE

  param P: U32

}
