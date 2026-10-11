# Identifiers are kept as 128-bit integers. The ID range of this instance ends
# past the largest one, so it is reported as an error where Scala, with
# arbitrary-precision identifiers, accepts it.

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
