# The default set and save opcodes of a parameter follow the previous opcode,
# as the spec says, also past 32 bits. Command B has opcode 0xFFFFFFFF, so
# the parameter has set opcode 0x100000000 and save opcode 0x100000001, and
# no opcode is duplicated.

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

  sync command A opcode 0

  sync command B opcode 0xFFFFFFFF

  param P: U32

}
