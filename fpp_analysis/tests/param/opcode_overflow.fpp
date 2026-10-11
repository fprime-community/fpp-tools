# Opcodes are kept as 128-bit integers. Command A has the opcode before the
# largest one, so the default set opcode of the parameter is the largest one,
# and its default save opcode is out of that range: the parameter is an
# error.

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
