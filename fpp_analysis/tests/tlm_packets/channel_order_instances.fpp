# Rust-only: the channels of a packet set are checked in the order of the
# instance names, not in the order of their ids
module Fw {
  port Time
  port Tlm
}

passive component C {
  time get port timeGetOut
  telemetry port tlmOut
  telemetry T: U32
}

instance a: C base id 0x200
instance b: C base id 0x100
deployment topology T {
  instance a
  instance b
  telemetry packets P {
    packet A group 0 {
    }
  }
}
