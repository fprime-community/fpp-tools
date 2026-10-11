# Identifiers are kept as 128-bit integers. The base identifier of this
# instance is the largest one, so its ID range, which holds two telemetry
# channel identifiers, ends past that range: the instance is an error.

module Fw {
  port Tlm
  port Time
}

passive component C {

  telemetry port tlmOut

  time get port timeGetOut

  telemetry T0: U32

  telemetry T1: U32

}

instance c: C base id 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF
