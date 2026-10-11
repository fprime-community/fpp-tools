# Rust-only: packets are checked for duplicate names in model order, whatever
# their ids
deployment topology T {
  instance c1
  telemetry packets P {
    packet A id 5 group 0 {
      c1.T
    }
    packet A id 1 group 0 {
    }
  }
}
