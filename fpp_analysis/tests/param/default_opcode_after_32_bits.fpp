# The default set and save opcodes of a parameter follow the previous opcode,
# as the spec says. fpp-check truncates that default to a 32-bit integer
# first, so it gives the parameter set opcode 0 here and reports a duplicate
# opcode.

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
