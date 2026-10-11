# Rust-only: deployment topologies are visited in model order, here the order
# of the input files
deployment topology A {
  instance c2
  telemetry packets Q {
    packet Q group 0 { c2.T }
    packet Q group 1 { c2.T }
  }
}
