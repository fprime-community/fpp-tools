# Rust-only: the channels of an instance are checked in declaration order,
# not in the order of their ids
module Fw {
  port Time
  port Tlm
}
passive component C {
  time get port timeGetOut
  telemetry port tlmOut
  telemetry T2: U32 id 5
  telemetry T1: U32 id 1
}
instance c1: C base id 0x100
deployment topology T {
  instance c1
  telemetry packets P {
    packet A group 0 {
    }
  }
}
