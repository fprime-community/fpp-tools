# Rust-only: a channel used in several packets is reported as used in the last
# packet in model order, whatever the packet ids
deployment topology T {
  instance c1
  telemetry packets P {
    packet A0 id 5 group 0 {
      c1.T
    }
    packet A1 id 1 group 0 {
      c1.T
    }
    packet A2 id 3 group 0 {
      c1.T
    }
  } omit {
    c1.T
  }
}
